use axum::{
    extract::{Path, Query, State},
    http::{header::CONTENT_TYPE, HeaderMap, StatusCode},
    response::IntoResponse,
};

const AVAILABLE_FORMATS: [mime::Mime; 2] = [mime::IMAGE_JPEG, mime::IMAGE_PNG];

#[derive(Clone)]
struct WarmContext {
    s3: aws_sdk_s3::Client,
    bucket_name: String,
}

type Error = (StatusCode, String);

#[derive(serde::Deserialize)]
struct Params {
    width: u32,
    height: u32,
}

async fn handler(
    State(WarmContext { s3, bucket_name }): State<WarmContext>,
    Query(Params { width, height }): Query<Params>,
    Path(path): Path<String>,
    headers: HeaderMap,
) -> Result<impl IntoResponse, Error> {
    let accept = match headers.get("accept") {
        Some(accept) => accept
            .to_str()
            .map_err(error(StatusCode::INTERNAL_SERVER_ERROR))?,
        None => "*/*",
    };
    let negotiated = accept
        .parse::<accept_header::Accept>()
        .map_err(error(StatusCode::BAD_REQUEST))?
        .negotiate(&AVAILABLE_FORMATS)
        .map_err(|e| (e, format!("available types: {AVAILABLE_FORMATS:?}")))?;
    let format = match (negotiated.type_(), negotiated.subtype()) {
        (mime::IMAGE, mime::PNG) => image::ImageFormat::Png,
        (mime::IMAGE, mime::JPEG) => image::ImageFormat::Jpeg,
        _ => unreachable!("we've already filtered to {AVAILABLE_FORMATS:?}"),
    };

    let in_buffer = s3
        .get_object()
        .bucket(&bucket_name)
        .key(&path)
        .send()
        .await
        .map_err(error(StatusCode::NOT_FOUND))?
        .body
        .collect()
        .await
        .map_err(error(StatusCode::INTERNAL_SERVER_ERROR))?
        .into_bytes();

    let image = image::load_from_memory(&in_buffer)
        .map_err(error(StatusCode::INTERNAL_SERVER_ERROR))?
        .resize_to_fill(width, height, image::imageops::Nearest);

    let mut out_buffer = std::io::BufWriter::new(std::io::Cursor::new(vec![]));
    image
        .write_to(&mut out_buffer, format)
        .map_err(error(StatusCode::INTERNAL_SERVER_ERROR))?;

    Ok((
        StatusCode::OK,
        [(CONTENT_TYPE, negotiated.to_string())],
        out_buffer
            .into_inner()
            .map_err(error(StatusCode::INTERNAL_SERVER_ERROR))?
            .into_inner(),
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

    let warm_context = WarmContext {
        s3: aws_sdk_s3::Client::new(&aws_config::from_env().load().await),
        bucket_name: std::env::var("BUCKET_NAME").expect("missing BUCKET_NAME env var"),
    };

    let app = axum::Router::new()
        .route("/*path", axum::routing::get(handler))
        .with_state(warm_context);

    lambda_http::run(app).await
}

fn error<E: ToString>(status: StatusCode) -> impl FnOnce(E) -> Error {
    move |e| (status, e.to_string())
}
