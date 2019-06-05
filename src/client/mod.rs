pub mod response;

pub use client::response::*;
pub use tokio_service::Service;

use std::future::Future;

use futures::{
    compat::{Future01CompatExt, Stream01CompatExt},
    future::{ok, err, FutureExt, TryFutureExt},
    stream::TryStreamExt,
};
use reqwest::{r#async::Client as HttpClient, StatusCode};

use message::Message;
use serde_json;


pub struct Client {
    http_client: HttpClient,
}

impl Client {
    /// Get a new instance of Client.
    pub fn new() -> Client {
        let http_client = HttpClient::builder()
            .use_rustls_tls()
            .build()
            .unwrap();
        Client { http_client, }
    }

    pub fn send(&self, message: Message) -> impl Future<Output = Result<FcmResponse, FcmError>> {
        let payload = serde_json::to_vec(&message.body).unwrap();

        let req = self.http_client
            .post("https://fcm.googleapis.com/fcm/send")
            .header("content-type", "application/json; encoding=utf-8")
            .header("content-length", format!("{}", payload.len() as u64))
            .header("authorization", format!("key={}", message.api_key).as_bytes())
            .body(payload);

        let send_request = req.send()
            .compat()
            .map_err(|_| response::FcmError::ServerError(None));

        let fcm_f = send_request.and_then(move |response| {
            let response_status = response.status().clone();
            let retry_after = response.headers()
                .get("retry-after")
                .and_then(|ra| ra.to_str().ok())
                .and_then(|ra| RetryAfter::from_str(ra));

            response
                .into_body()
                .compat()
                .map_err(|_| FcmError::ServerError(None))
                .try_concat()
                .and_then(move |body_chunk| {
                    if let Ok(body) = String::from_utf8(body_chunk.to_vec()) {
                        match response_status {
                            StatusCode::OK => {
                                let fcm_response: FcmResponse = serde_json::from_str(&body).unwrap();

                                match fcm_response.error {
                                    Some(ErrorReason::Unavailable) =>
                                        err(response::FcmError::ServerError(retry_after)),
                                    Some(ErrorReason::InternalServerError) =>
                                        err(response::FcmError::ServerError(retry_after)),
                                    _ =>
                                        ok(fcm_response)
                                }
                            },
                            StatusCode::UNAUTHORIZED =>
                                err(response::FcmError::Unauthorized),
                            StatusCode::BAD_REQUEST =>
                                err(response::FcmError::InvalidMessage("Bad Request".to_string())),
                            status if status.is_server_error() =>
                                err(response::FcmError::ServerError(retry_after)),
                            _ =>
                                err(response::FcmError::InvalidMessage("Unknown Error".to_string()))
                        }
                    } else {
                        err(response::FcmError::InvalidMessage("Unknown Error".to_string()))
                    }
                })
        });

        fcm_f.boxed()
    }
}
