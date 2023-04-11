use axum::{
    extract::{Path, Query, State},
    http::{header, HeaderMap, StatusCode},
    response::IntoResponse,
    routing::get,
    Json, Router,
};

async fn root<'a>(
    headers: HeaderMap,
    query: Query<Params>,
    Path((path,)): Path<(String,)>,
    State(WarmContext {
        s3_client,
        environment,
    }): State<WarmContext>,
) -> Response<'a, impl IntoResponse> {
    let content_type = match headers.get("accept") {
        Some(t) => t.to_str().or_else(Error::op(
            StatusCode::BAD_REQUEST,
            "Couldn't read accept header.",
        ))?,
        None => "image/png",
    };

    let format = match content_type.split_once("/").ok_or_else(Error::err(
        StatusCode::BAD_REQUEST,
        "invalid `accept` header.",
    ))? {
        ("image", "png") => Ok(image::ImageFormat::Png),
        ("image", "webp") => Ok(image::ImageFormat::WebP),
        ("image", "jpg" | "jpeg") => Ok(image::ImageFormat::Jpeg),
        _ => Err(Error::err(
            StatusCode::BAD_REQUEST,
            format!("invalid `accept` header"),
        )()),
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
        .or_else(Error::op(
            StatusCode::NOT_FOUND,
            format!("Couldn't find image at {:?}", &path),
        ))?;

    let in_buffer = object
        .body
        .collect()
        .await
        .or_else(Error::op(
            StatusCode::INTERNAL_SERVER_ERROR,
            "Couldn't collect object body.",
        ))?
        .into_bytes();

    let image = image::load_from_memory(&in_buffer)
        .or_else(Error::op(
            StatusCode::INTERNAL_SERVER_ERROR,
            "Couldn't load image from memory.",
        ))?
        .resize_exact(width, height, image::imageops::Gaussian);

    let mut out_buffer = std::io::BufWriter::new(std::io::Cursor::new(Vec::new()));
    image.write_to(&mut out_buffer, format).or_else(Error::op(
        StatusCode::INTERNAL_SERVER_ERROR,
        "Couldn't encode image. [0]",
    ))?;

    let bytes = out_buffer
        .into_inner()
        .or_else(Error::op(
            StatusCode::INTERNAL_SERVER_ERROR,
            "Couldn't encode image. [1]",
        ))?
        .into_inner();

    Ok((
        StatusCode::OK,
        [(header::CONTENT_TYPE, content_type.to_string())],
        bytes,
    ))
}

#[derive(Debug, serde::Deserialize)]
struct Params {
    width: Option<u32>,
    height: Option<u32>,
}

#[derive(serde::Serialize)]
struct Error {
    #[serde(rename = "type")]
    error_type: Option<String>,
    message: String,
}

type Response<'a, T> = Result<
    T,
    (
        StatusCode,
        [(axum::http::HeaderName, &'a str); 1],
        Json<Error>,
    ),
>;

impl<'a> Error {
    fn op<T, Input: ToString>(
        code: StatusCode,
        message: impl ToString,
    ) -> impl FnOnce(
        Input,
    ) -> Result<
        T,
        (
            StatusCode,
            [(axum::http::HeaderName, &'a str); 1],
            Json<Self>,
        ),
    > {
        move |e| {
            Err((
                code,
                [(header::CONTENT_TYPE, "application/error+json")],
                Json::from(Self {
                    error_type: Some(e.to_string()),
                    message: message.to_string(),
                }),
            ))
        }
    }

    fn err(
        code: StatusCode,
        message: impl ToString,
    ) -> impl FnOnce() -> (
        StatusCode,
        [(axum::http::HeaderName, &'a str); 1],
        Json<Self>,
    ) {
        move || {
            (
                code,
                [(header::CONTENT_TYPE, "application/error+json")],
                Json::from(Self {
                    error_type: None,
                    message: message.to_string(),
                }),
            )
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
