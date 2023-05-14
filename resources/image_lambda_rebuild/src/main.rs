use axum::{
    extract::{Path, Query, State},
    http::{header::CONTENT_TYPE, HeaderMap, StatusCode},
    response::IntoResponse,
};
use std::io::{BufWriter, Cursor};

const AVAILABLE_FORMATS: [mime::Mime; 2] = [mime::IMAGE_JPEG, mime::IMAGE_PNG];

#[derive(serde::Deserialize)]
struct Params {
    width: u32,
    height: u32,
}

#[derive(Clone)]
struct WarmContext {
    s3: aws_sdk_s3::Client,
    bucket_name: String,
}

macro_rules! error {
    ($status:ident) => {
        |e| Err((StatusCode::$status, e.to_string()))
    };
}

async fn handler(
    State(WarmContext { s3, bucket_name }): State<WarmContext>,
    Path(path): Path<String>,
    Query(Params { width, height }): Query<Params>,
    headers: HeaderMap,
) -> Result<impl IntoResponse, (StatusCode, String)> {
    let accept = match headers.get("accept") {
        Some(accept) => accept.to_str().or_else(error!(BAD_REQUEST))?,
        None => "*/*",
    };
    let negotiated = accept
        .parse::<accept_header::Accept>()
        .or_else(error!(BAD_REQUEST))?
        .negotiate(&AVAILABLE_FORMATS)
        .or_else(|e| Err((e, "couldn't find type from available formats".into())))?;
    let negotiated_format = match (negotiated.type_(), negotiated.subtype()) {
        (mime::IMAGE, mime::JPEG) => image::ImageFormat::Jpeg,
        (mime::IMAGE, mime::PNG) => image::ImageFormat::Png,
        _ => unreachable!("we've already filtered our mime types to {AVAILABLE_FORMATS:?}"),
    };

    let in_buffer = s3
        .get_object()
        .bucket(&bucket_name)
        .key(&path)
        .send()
        .await
        .or_else(error!(NOT_FOUND))?
        .body
        .collect()
        .await
        .or_else(error!(INTERNAL_SERVER_ERROR))?
        .into_bytes();

    let image = image::load_from_memory(&in_buffer).or_else(error!(INTERNAL_SERVER_ERROR))?;

    let mut out_buffer = BufWriter::new(Cursor::new(vec![]));
    image
        .resize(width, height, image::imageops::Lanczos3)
        .write_to(&mut out_buffer, negotiated_format)
        .or_else(error!(INTERNAL_SERVER_ERROR))?;

    Ok((
        StatusCode::OK,
        [(CONTENT_TYPE, negotiated.to_string())],
        out_buffer.buffer().to_owned(),
    ))
}

#[tokio::main]
async fn main() -> Result<(), lambda_http::Error> {
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        // disable printing the name of the module in every log line.
        .with_target(false)
        // disabling time is handy because CloudWatch will add the ingestion time.
        .without_time()
        .init();

    let state = WarmContext {
        s3: aws_sdk_s3::Client::new(&aws_config::from_env().load().await),
        bucket_name: std::env::var("BUCKET_NAME").expect("missing BUCKET_NAME env var"),
    };

    let app = axum::Router::new()
        .route("/*path", axum::routing::get(handler))
        .with_state(state);

    lambda_http::run(app).await
}
