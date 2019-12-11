pub mod response;

use bytes::buf::BufExt;
use futures::future::TryFutureExt;
use hyper::{Body, client::connect::Connection, Request, service::Service, StatusCode, Uri};
use tokio::io::{AsyncRead, AsyncWrite};

pub use crate::client::response::*;
use crate::message::Message;


pub struct Client<S>
where
    S: Service<Uri> + Clone + Send + Sync + 'static,
    S::Response: Connection + AsyncRead + AsyncWrite + Send + Unpin + 'static,
    S::Future: Send + Unpin + 'static,
    S::Error: Into<Box<dyn std::error::Error + Send + Sync>>,
{
    http_client: hyper::Client<S, Body>,
}

impl<S> Client<S>
where
    S: Service<Uri> + Clone + Send + Sync + 'static,
    S::Response: Connection + AsyncRead + AsyncWrite + Send + Unpin + 'static,
    S::Future: Send + Unpin + 'static,
    S::Error: Into<Box<dyn std::error::Error + Send + Sync>>,
{
    /// Get a new instance of Client.
    pub fn new(http_client: hyper::Client<S, Body>) -> Client<S> {
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

        match response_status {
            StatusCode::OK => {
                let body_buf = hyper::body::aggregate(response.into_body()).await
                    .map_err(|_| response::FcmError::InvalidMessage("Failed to read response".to_string()))?;
                let fcm_response: FcmResponse = serde_json::from_reader(body_buf.reader())
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
