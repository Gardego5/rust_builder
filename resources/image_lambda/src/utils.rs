use crate::{error, errors::Error, WarmContext};
use lambda_http::RequestExt;
use std::str::FromStr;

pub async fn get_image<'a>(
    path: &'a str,
    ctx: &'a WarmContext,
) -> Result<image::DynamicImage, Error> {
    let buffer = &ctx
        .s3_client
        .get_object()
        .bucket(&ctx.env.bucket_name)
        .key(path)
        .send()
        .await
        .or_else(error!(NOT_FOUND "could not retrieve object from s3"))?
        .body
        .collect()
        .await
        .or_else(error!("could not collect byte stream from s3 image"))?
        .into_bytes();

    Ok(image::load_from_memory(buffer).or_else(error!("could not load from image memory"))?)
}

pub fn write_image_to_bytes(
    image: image::DynamicImage,
    format: image::ImageFormat,
) -> Result<Vec<u8>, Error> {
    let mut out_buffer = std::io::BufWriter::new(std::io::Cursor::new(Vec::new()));

    image
        .write_to(&mut out_buffer, format)
        .or_else(error!("could not convert image into bytes"))?;

    Ok(out_buffer.buffer().to_vec())
}

pub fn get_required_query_param<T: FromStr>(
    req: &lambda_http::Request,
    key: &str,
) -> Result<T, Error>
where
    <T as FromStr>::Err: ToString,
{
    req.query_string_parameters()
        .first(key)
        .ok_or(error!(raw BAD_REQUEST format!("you must provide a {key} parameter")))?
        .parse::<T>()
        .or_else(error!(BAD_REQUEST format!("{key} could not be parsed as {}", std::any::type_name::<T>())))
}
