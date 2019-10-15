#![doc(html_root_url = "https://panicbit.github.io/fcm-rust/fcm/")]
//! fcm
//! ===
//!
//! A client for asynchronous sending of Firebase Cloud Messages, or Push Notifications.
//!
//! # Examples:
//!
//! To send out a FCM Message with some custom data:
//!
//! ```no_run
//! # use std::collections::HashMap;
//! # use futures::{future::lazy, Future};
//! # fn main() {
//! let client = fcm::Client::new().unwrap();
//!
//! let mut map = HashMap::new();
//! map.insert("message", "Howdy!");
//!
//! let mut builder = fcm::MessageBuilder::new("<FCM API Key>", "<registration id>");
//! builder.data(&map);
//!
//! let payload = builder.finalize();
//!
//! tokio::run(lazy(move || {
//!     client
//!         .send(payload)
//!         .map(|response| {
//!             println!("Sent: {:?}", response);
//!         }).map_err(|error| {
//!             println!("Error: {:?}", error)
//!         })
//! }));
//! # }
//! ```
//!
//! To send a message using FCM Notifications, we first build the notification:
//!
//! ```rust
//! # fn main() {
//! let mut builder = fcm::NotificationBuilder::new();
//! builder.title("Hey!");
//! builder.body("Do you want to catch up later?");
//! let notification = builder.finalize();
//! # }
//! ```
//!
//! And then set it in the message, before sending it:
//!
//! ```no_run
//! # use futures::{future::lazy, Future};
//! # fn main() {
//! let client = fcm::Client::new().unwrap();
//!
//! let mut notification_builder = fcm::NotificationBuilder::new();
//! notification_builder.title("Hey!");
//! notification_builder.body("Do you want to catch up later?");
//!
//! let notification = notification_builder.finalize();
//! let mut message_builder = fcm::MessageBuilder::new("<FCM API Key>", "<registration id>");
//! message_builder.notification(notification);
//!
//! let payload = message_builder.finalize();
//!
//! tokio::run(lazy(move || {
//!     client
//!         .send(payload)
//!         .map(|response| {
//!             println!("Sent: {:?}", response);
//!         }).map_err(|error| {
//!             println!("Error: {:?}", error)
//!         })
//! }));
//! # }
//! ```

#[allow(unused_imports)]

mod message;
pub use message::*;
mod notification;
pub use notification::*;
mod client;
pub use client::*;

pub use client::response::FcmError as Error;
