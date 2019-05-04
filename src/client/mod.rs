pub mod response;

pub use client::response::*;
pub use tokio_service::Service;

use std::future::Future;

use futures::{
    compat::{Future01CompatExt, Stream01CompatExt},
    future::{ok, err, FutureExt, TryFutureExt},
    stream::TryStreamExt,
};
use http::{
    header::{AUTHORIZATION, CONTENT_TYPE, CONTENT_LENGTH, RETRY_AFTER},
};
use hyper::{
    Request,
    StatusCode,
    Body,
    client::Client as HttpClient,
};

use message::Message;
use serde_json;


pub struct Client<C> {
    http_client: HttpClient<C>,
}

impl<C: 'static> Client<C> where C: hyper::client::connect::Connect {
    /// Get a new instance of Client.
    pub fn new(connector: C) -> Client<C> {
        let mut http_client = HttpClient::builder();
        http_client.keep_alive(true);
        Client {
            http_client: http_client.build(connector),
        }
    }

    pub fn send(&self, message: Message) -> impl Future<Output = Result<FcmResponse, FcmError>> {
        let payload = serde_json::to_vec(&message.body).unwrap();

        let mut builder = Request::builder();

        builder.method("POST");
        builder.header(CONTENT_TYPE, "application/json");
        builder.header(CONTENT_LENGTH, format!("{}", payload.len() as u64).as_bytes());
        builder.header(AUTHORIZATION, format!("key={}", message.api_key).as_bytes());
        builder.uri("https://fcm.googleapis.com/fcm/send");

        let request = builder.body(Body::from(payload)).unwrap();

        let send_request = self
            .http_client
            .request(request)
            .compat()
            .map_err(|_| response::FcmError::ServerError(None));

        let fcm_f = send_request.and_then(move |response| {
            let response_status = response.status().clone();
            let retry_after = response.headers()
                .get(RETRY_AFTER)
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
