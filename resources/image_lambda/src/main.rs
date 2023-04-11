use axum::{
    extract::{Path, State},
    http::{header, StatusCode},
    response::IntoResponse,
    routing::get,
    Json, Router,
};
use lambda_http::aws_lambda_events::serde_json::json;

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
    println!("Hello Lambda World");

    let objects = s3_client
        .list_objects_v2()
        .bucket(&environment.bucket_name)
        .send()
        .await
        .or_else(Error::op(StatusCode::INTERNAL_SERVER_ERROR, ""))?
        .contents()
        .ok_or_else(Error::err(StatusCode::BAD_REQUEST, "Contents were None."))?
        .to_owned();

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

    let meta = ob
        .metadata()
        .ok_or_else(Error::err(
            StatusCode::INTERNAL_SERVER_ERROR,
            "No metadata found.",
        ))?
        .clone();

    println!("{meta:?}");

    Ok((
        StatusCode::OK,
        [(header::CONTENT_TYPE, "application/json")],
        Json::from(json!({
            "objects": objects.iter().map(|o| format!("{o:?}")).collect::<Vec<String>>(),
            "meta": meta
        })),
    ))
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
