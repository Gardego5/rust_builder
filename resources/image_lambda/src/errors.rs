use lambda_http::{http::StatusCode, Body, Response};
use serde_json::Value;

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

#[macro_export]
macro_rules! error {
    (raw $status:ident, $message:literal) => {
        crate::errors::Error(
            lambda_http::http::StatusCode::$status,
            serde_json::json!({ "message": $message })
        )
    };
    (raw $message:literal) => {
        error!(raw INTERNAL_SERVER_ERROR, $message)
    };
    ($error_type:ty, $status:ident) => {
        |error: $error_type| {
            Err(crate::errors::Error(
                lambda_http::http::StatusCode::$status,
                serde_json::json!({ "error": error.to_string() }),
            ))
        }
    };
    ($error_type:ty, $status:ident, $message:literal) => {
        |error: $error_type| {
            Err(crate::errors::Error(
                lambda_http::http::StatusCode::$status,
                serde_json::json!({ "error": error.to_string(), "message": $message })
            ))
        }
    };
    () => {
        error!(INTERNAL_SERVER_ERROR)
    };
    ($message:literal) => {
        error!(INTERNAL_SERVER_ERROR, $message)
    };
    ($status:ident) => {
        error!(_, $status)
    };
    ($status:ident, $message:literal) => {
        error!(_, $status, $message)
    };
    ($error_type:ty) => {
        error!($error_type, INTERNAL_SERVER_ERROR)
    };
    ($error_type:ty, $message:literal) => {
        error!($error_type, INTERNAL_SERVER_ERROR, $message)
    };
}
