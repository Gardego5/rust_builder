use axum::{
    extract::{Path, State},
    http::{header, StatusCode},
    response::IntoResponse,
    routing::get,
    Json, Router,
};

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

async fn root<'a>(
    Path((path,)): Path<(String,)>,
    State(WarmContext {
        s3_client,
        environment,
    }): State<WarmContext>,
) -> Response<'a, impl IntoResponse> {
    let ob = s3_client
        .get_object()
        .bucket(&environment.bucket_name)
        .key(&path)
        .send()
        .await
        .or_else(Error::op(
            StatusCode::NOT_FOUND,
            format!("Couldn't find image at {:?}", &path),
        ))?;

    let buffer = ob
        .body
        .collect()
        .await
        .or_else(Error::op(
            StatusCode::INTERNAL_SERVER_ERROR,
            "Couldn't collect object body.",
        ))?
        .into_bytes();

    let resized_image = image::load_from_memory(&buffer)
        .or_else(Error::op(
            StatusCode::INTERNAL_SERVER_ERROR,
            "Couldn't load image from memory.",
        ))?
        .resize_exact(100, 100, image::imageops::Gaussian);

    let mut buffer = std::io::BufWriter::new(std::io::Cursor::new(Vec::new()));
    resized_image
        .write_to(&mut buffer, image::ImageFormat::Png)
        .or_else(Error::op(
            StatusCode::INTERNAL_SERVER_ERROR,
            "Couldn't encode image. [0]",
        ))?;

    let bytes = buffer
        .into_inner()
        .or_else(Error::op(
            StatusCode::INTERNAL_SERVER_ERROR,
            "Couldn't encode image. [1]",
        ))?
        .into_inner();

    Ok((StatusCode::OK, [(header::CONTENT_TYPE, "image/png")], bytes))
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
