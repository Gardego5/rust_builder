use accept_header::Accept;
use axum::{
    extract::{Path, Query, State},
    http::{header, HeaderMap, StatusCode},
    response::IntoResponse,
    routing::get,
    Json, Router,
};

const AVAILABLE: &[mime::Mime] = &[mime::IMAGE_PNG, mime::IMAGE_JPEG];

async fn root<'a>(
    headers: HeaderMap,
    query: Query<Params>,
    Path((path,)): Path<(String,)>,
    State(WarmContext {
        s3_client,
        environment,
    }): State<WarmContext>,
) -> RR<impl IntoResponse> {
    let accept_header: Accept = headers
        .get("accept")
        .unwrap_or(&axum::http::HeaderValue::from_static("*/*"))
        .to_str()
        .or_else(EJ::op(
            StatusCode::BAD_REQUEST,
            "couldn't parse accept header [0]",
        ))?
        .parse()
        .or_else(EJ::op(
            StatusCode::BAD_REQUEST,
            "couldn't parse accept header [1]",
        ))?;

    let best = accept_header.negotiate(AVAILABLE).or_else(EJ::op(
        StatusCode::UNSUPPORTED_MEDIA_TYPE,
        "Couldn't find a matching mime type.",
    ))?;

    let format = match (best.type_(), best.subtype()) {
        (mime::IMAGE, mime::PNG) => Ok(image::ImageFormat::Png),
        (mime::IMAGE, mime::JPEG) => Ok(image::ImageFormat::Jpeg),
        _ => EJ::new(
            StatusCode::UNSUPPORTED_MEDIA_TYPE,
            "Couldn't find a matching image format.",
        )
        .into(),
    }?;

    let (width, height) = match query.0 {
        Params {
            width: Some(w),
            height: Some(h),
        } => (w, h),
        _ => (100, 100),
    };

    let object = s3_client
        .get_object()
        .bucket(&environment.bucket_name)
        .key(&path)
        .send()
        .await
        .or_else(EJ::op(
            StatusCode::NOT_FOUND,
            format!("Couldn't find image at {:?}", &path),
        ))?;

    let in_buffer = object
        .body
        .collect()
        .await
        .or_else(EJ::op(
            StatusCode::INTERNAL_SERVER_ERROR,
            "Couldn't collect object body.",
        ))?
        .into_bytes();

    let image = image::load_from_memory(&in_buffer)
        .or_else(EJ::op(
            StatusCode::INTERNAL_SERVER_ERROR,
            "Couldn't load image from memory.",
        ))?
        .resize_exact(width, height, image::imageops::Gaussian);

    let mut out_buffer = std::io::BufWriter::new(std::io::Cursor::new(Vec::new()));
    image.write_to(&mut out_buffer, format).or_else(EJ::op(
        StatusCode::INTERNAL_SERVER_ERROR,
        "Couldn't encode image. [0]",
    ))?;

    let image_bytes = out_buffer
        .into_inner()
        .or_else(EJ::op(
            StatusCode::INTERNAL_SERVER_ERROR,
            "Couldn't encode image. [1]",
        ))?
        .into_inner();

    Ok((
        StatusCode::OK,
        [(header::CONTENT_TYPE, best.to_string())],
        image_bytes,
    ))
}

#[derive(Debug, serde::Deserialize)]
struct Params {
    width: Option<u32>,
    height: Option<u32>,
}

#[derive(serde::Serialize)]
struct EJ {
    #[serde(skip)]
    code: StatusCode,
    #[serde(rename = "type")]
    error_type: Option<String>,
    message: String,
}
type ER = (StatusCode, [(axum::http::HeaderName, String); 1], Json<EJ>);
type RR<T> = Result<T, ER>;

impl Into<ER> for EJ {
    fn into(self) -> ER {
        (
            self.code,
            [(
                header::CONTENT_TYPE,
                String::from("application/problem+json"),
            )],
            Json::from(self),
        )
    }
}

impl<T> Into<RR<T>> for EJ {
    fn into(self) -> Result<T, ER> {
        Err(self.into())
    }
}

impl EJ {
    fn op<T, I: ToString>(code: StatusCode, message: impl ToString) -> impl FnOnce(I) -> RR<T> {
        move |e| {
            Self {
                code,
                error_type: Some(e.to_string()),
                message: message.to_string(),
            }
            .into()
        }
    }

    fn new(code: StatusCode, message: impl ToString) -> Self {
        Self {
            code,
            error_type: None,
            message: message.to_string(),
        }
    }
}

#[derive(Clone, Debug)]
struct Environment {
    bucket_name: String,
}
impl Environment {
    fn load() -> Result<Self, lambda_http::Error> {
        Ok(Self {
            bucket_name: std::env::var("BUCKET_NAME")?,
        })
    }
}

#[derive(Clone)]
struct WarmContext {
    s3_client: aws_sdk_s3::Client,
    environment: Environment,
}

#[tokio::main]
async fn main() -> Result<(), lambda_http::Error> {
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .with_ansi(false)
        .with_target(false)
        .without_time()
        .init();

    let sdk_config = aws_config::from_env().load().await;

    let state = WarmContext {
        s3_client: aws_sdk_s3::Client::new(&sdk_config),
        environment: Environment::load()?,
    };

    let app = Router::new().route("/*path", get(root)).with_state(state);

    lambda_http::run(app).await
}
