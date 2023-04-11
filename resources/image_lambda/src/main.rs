use axum::{
    extract::State,
    http::{header, StatusCode},
    response::IntoResponse,
    routing::get,
    Json, Router,
};
use lambda_http::aws_lambda_events::serde_json::json;

#[derive(serde::Serialize)]
struct Error {
    #[serde(rename = "type")]
    _type: String,
    message: String,
}

type Response<T> = Result<T, (StatusCode, Json<Error>)>;
fn error<T, Input: ToString>(
    code: StatusCode,
    message: impl ToString,
) -> impl FnOnce(Input) -> Result<T, (StatusCode, Json<Error>)> {
    move |e| {
        Err((
            code,
            // [(header::CONTENT_TYPE, "application/problem+json")],
            Json::from(Error {
                _type: e.to_string(),
                message: message.to_string(),
            }),
        ))
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

async fn root(
    State(WarmContext {
        s3_client,
        environment,
    }): State<WarmContext>,
) -> Response<impl IntoResponse> {
    let objects = s3_client
        .list_objects_v2()
        .bucket(environment.bucket_name)
        .send()
        .await
        .or_else(error(StatusCode::INTERNAL_SERVER_ERROR, "Oops"))?
        .contents()
        .ok_or_else(|| "")
        .or_else(error(StatusCode::BAD_REQUEST, "OOOS"))?
        .to_owned();

    Ok((
        StatusCode::OK,
        [(header::CONTENT_TYPE, "text/plain")],
        Json::from(
            json!({ "objects": objects.iter().map(|o| format!("{o:?}")).collect::<Vec<String>>() }),
        ),
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

    let app = Router::new().route("/", get(root)).with_state(state);

    lambda_http::run(app).await
}
