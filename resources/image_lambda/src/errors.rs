use crate::WarmContext;
use lambda_http::{http::StatusCode, Body, Response};
use serde_json::Value;
use std::future::Future;

pub struct Error(pub StatusCode, pub Value);
impl TryInto<Response<Body>> for Error {
    type Error = lambda_http::Error;
    fn try_into(self) -> Result<Response<Body>, Self::Error> {
        let Error(status, json_error) = self;
        Ok(Response::builder()
            .status(status)
            .header("Content-Type", "application/problem+json")
            .body(Body::Text(json_error.to_string()))?)
    }
}

pub async fn handler<'a, F, Fut>(
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

#[macro_export]
macro_rules! error {
    (json $status:ident $json:tt) => {
        crate::errors::Error(
            lambda_http::http::StatusCode::$status,
            serde_json::json!($json),
        )
    };
    (raw $status:ident $title:expr, $detail:expr) => {
        error!(json $status { "title": $title, "detail": $detail })
    };
    (raw $status:ident $title:expr) => {
        error!(json $status { "title": $title })
    };
    ($status:ident) => {
        |error| Err(error!{raw $status error.to_string()})
    };
    ($status:ident $message:expr) => {
        |error| Err(error!{raw $status error.to_string(), $message})
    };
    ($message:expr) => {
        |error| Err(error!{raw INTERNAL_SERVER_ERROR error.to_string(), $message})
    };
    () => {
        |error| Err(error!{raw INTERNAL_SERVER_ERROR error.to_string()})
    };
}
