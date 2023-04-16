use std::future::Future;

use accept_header::Accept;
use errors::Error;
use lambda_http::{http::StatusCode, Body, RequestExt, Response};
use serde_json::json;
use utils::{get_image, write_image_to_bytes};

mod errors;
mod utils;

pub struct Env {
    pub bucket_name: String,
}

pub struct WarmContext {
    pub s3_client: aws_sdk_s3::Client,
    pub env: Env,
}

#[tokio::main]
async fn main() -> Result<(), lambda_http::Error> {
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .with_ansi(false)
        .with_target(false)
        .without_time()
        .init();

    let ctx: WarmContext = WarmContext {
        s3_client: aws_sdk_s3::Client::new(&aws_config::from_env().load().await),
        env: Env {
            bucket_name: std::env::var("BUCKET_NAME")?,
        },
    };

    lambda_http::run(lambda_http::service_fn(|req| {
        error_handler(req, &ctx, handler)
    }))
    .await
}

async fn error_handler<'a, F, Fut>(
    req: lambda_http::Request,
    ctx: &'a WarmContext,
    handler: F,
) -> Result<Response<Body>, lambda_http::Error>
where
    F: FnOnce(lambda_http::Request, &'a WarmContext) -> Fut,
    Fut: Future<Output = Result<Response<Body>, Error>>,
{
    match handler(req, ctx).await {
        Ok(result) => Ok(result),
        Err(error) => Ok(error.try_into()?),
    }
}

async fn handler(req: lambda_http::Request, ctx: &WarmContext) -> Result<Response<Body>, Error> {
    let (width, height) = (
        req.query_string_parameters()
            .first("width")
            .ok_or(error!(raw BAD_REQUEST, "you must provide a 'width' parameter"))?
            .parse::<u32>()
            .or_else(error!("'width' should be a number"))?,
        req.query_string_parameters()
            .first("height")
            .ok_or(error!(raw BAD_REQUEST, "you must provide a 'height' parameter"))?
            .parse::<u32>()
            .or_else(error!("'height' should be a number"))?,
    );

    let content_type: mime::Mime = req
        .headers()
        .get("accept")
        .unwrap_or(&lambda_http::http::HeaderValue::from_static("*/*"))
        .to_str()
        .or_else(error!())?
        .parse::<Accept>()
        .or_else(error!(BAD_REQUEST, "couldn't read accept header"))?
        .negotiate(&[mime::IMAGE_PNG, mime::IMAGE_JPEG])
        .or_else(error!(
            UNSUPPORTED_MEDIA_TYPE,
            "couldn't find suitable accept header"
        ))?;

    let format = match (content_type.type_(), content_type.subtype()) {
        (mime::IMAGE, mime::PNG) => image::ImageFormat::Png,
        (mime::IMAGE, mime::JPEG) => image::ImageFormat::Jpeg,
        _ => return Err(Error(StatusCode::UNSUPPORTED_MEDIA_TYPE, json!({}))),
    };

    let path = req.uri().path();

    let image = get_image(path, ctx)
        .await?
        .resize(width, height, image::imageops::Lanczos3);

    let image_bytes = write_image_to_bytes(image, format)?;

    Ok(Response::builder()
        .header("Content-Type", content_type.to_string())
        .body(Body::Binary(image_bytes))
        .or_else(error!("could not build response"))?)
}
