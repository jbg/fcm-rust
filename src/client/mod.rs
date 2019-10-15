pub mod response;

use futures::{
    future::TryFutureExt,
    stream::TryStreamExt,
};
use hyper::{client::connect::Connect, Request, StatusCode};
pub use tokio_service::Service;

pub use crate::client::response::*;
use crate::message::Message;


pub struct Client<T: Connect + 'static> {
    http_client: hyper::Client<T>,
}

impl<T: Connect + 'static> Client<T> {
    /// Get a new instance of Client.
    pub fn new(http_client: hyper::Client<T>) -> Client<T> {
        Client { http_client }
    }

    pub async fn send<'a>(&'a self, message: Message<'a>) -> Result<FcmResponse, FcmError> {
        let payload = serde_json::to_vec(&message.body)
            .map_err(|_| response::FcmError::InvalidMessage("Can't serialise JSON".to_string()))?;

        let req = Request::builder()
            .method("POST")
            .uri("https://fcm.googleapis.com/fcm/send")
            .header("content-type", "application/json; encoding=utf-8")
            .header("content-length", format!("{}", payload.len() as u64))
            .header("authorization", format!("key={}", message.api_key).as_bytes())
            .body(payload.into())
            .map_err(|_| response::FcmError::InvalidMessage("Can't build HTTP request".to_string()))?;

        let response = self.http_client.request(req)
            .map_err(|_| response::FcmError::ServerError(None))
            .await?;

        let response_status = response.status().clone();
        let retry_after = response.headers()
            .get("retry-after")
            .and_then(|ra| ra.to_str().ok())
            .and_then(|ra| RetryAfter::from_str(ra));

        let body_chunk = response
            .into_body()
            .map_err(|_| FcmError::ServerError(None))
            .try_concat()
            .await?;

        let body = String::from_utf8(body_chunk.to_vec())
            .map_err(|_| response::FcmError::InvalidMessage("Unknown Error".to_string()))?;

        match response_status {
            StatusCode::OK => {
                let fcm_response: FcmResponse = serde_json::from_str(&body)
                    .map_err(|_| response::FcmError::InvalidMessage("Unknown Error".to_string()))?;
                match fcm_response.error {
                    Some(ErrorReason::Unavailable) =>
                        Err(response::FcmError::ServerError(retry_after)),
                    Some(ErrorReason::InternalServerError) =>
                        Err(response::FcmError::ServerError(retry_after)),
                    _ =>
                        Ok(fcm_response)
                }
            },
            StatusCode::UNAUTHORIZED =>
                Err(response::FcmError::Unauthorized),
            StatusCode::BAD_REQUEST =>
                Err(response::FcmError::InvalidMessage("Bad Request".to_string())),
            status if status.is_server_error() =>
                Err(response::FcmError::ServerError(retry_after)),
            _ =>
                Err(response::FcmError::InvalidMessage("Unknown Error".to_string()))
        }
    }
}
