// Copyright 2020 - developers of the `grammers` project.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// https://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or https://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.
//pub mod iterators;
//mod parsers;
//mod auth;
//mod chats;
//mod messages;
//mod updates;

use futures::future::FutureExt as _;
use futures::{future, pin_mut};
use grammers_mtproto::{mtp, transport};
use grammers_mtsender::{self as sender, InvocationError, Sender};
use std::net::Ipv4Addr;
use tokio::sync::{mpsc, oneshot};

/// Socket addresses to Telegram datacenters, where the index into this array
/// represents the data center ID.
///
/// The addresses were obtained from the `static` addresses through a call to
/// `functions::help::GetConfig`.
const DC_ADDRESSES: [(Ipv4Addr, u16); 6] = [
    (Ipv4Addr::new(149, 154, 167, 51), 443), // default (2)
    (Ipv4Addr::new(149, 154, 175, 53), 443),
    (Ipv4Addr::new(149, 154, 167, 51), 443),
    (Ipv4Addr::new(149, 154, 175, 100), 443),
    (Ipv4Addr::new(149, 154, 167, 92), 443),
    (Ipv4Addr::new(91, 108, 56, 190), 443),
];

/// When no locale is found, use this one instead.
const DEFAULT_LOCALE: &str = "en";

struct Request {
    request: Vec<u8>,
    response: oneshot::Sender<oneshot::Receiver<Result<Vec<u8>, InvocationError>>>,
}

/// A client capable of connecting to Telegram and invoking requests.
pub struct Client {
    //config: Config,
    sender: Sender<transport::Full, mtp::Encrypted>,

    /// The stored phone and its hash from the last `request_login_code` call.
    //last_phone_hash: Option<(String, String)>,

    /// The user identifier of the currently logged-in user.
    //user_id: Option<i32>,
    handle_tx: mpsc::UnboundedSender<Request>,
    handle_rx: mpsc::UnboundedReceiver<Request>,
}

#[derive(Clone)]
pub struct ClientHandle {
    tx: mpsc::UnboundedSender<Request>,
}

/// Configuration required to create a [`Client`] instance.
///
/// [`Client`]: struct.Client.html
pub struct Config {
    /// Session storage where data should persist, such as authorization key, server address,
    /// and other required information by the client.
    //pub session: Session,

    /// Developer's API ID, required to interact with the Telegram's API.
    ///
    /// You may obtain your own in <https://my.telegram.org/auth>.
    pub api_id: i32,

    /// Developer's API hash, required to interact with Telegram's API.
    ///
    /// You may obtain your own in <https://my.telegram.org/auth>.
    pub api_hash: String,

    /// Additional initialization parameters that can have sane defaults.
    pub params: InitParams,
}

/// Optional initialization parameters, required when initializing a connection to Telegram's
/// API.
pub struct InitParams {
    pub device_model: String,
    pub system_version: String,
    pub app_version: String,
    pub system_lang_code: String,
    pub lang_code: String,
}

impl Default for InitParams {
    fn default() -> Self {
        let info = os_info::get();

        let mut system_lang_code = locate_locale::system();
        if system_lang_code.is_empty() {
            system_lang_code.push_str(DEFAULT_LOCALE);
        }

        let mut lang_code = locate_locale::user();
        if lang_code.is_empty() {
            lang_code.push_str(DEFAULT_LOCALE);
        }

        Self {
            device_model: format!("{} {}", info.os_type(), info.bitness()),
            system_version: info.version().to_string(),
            app_version: env!("CARGO_PKG_VERSION").to_string(),
            system_lang_code,
            lang_code,
        }
    }
}

impl Client {
    pub async fn connect() -> Self {
        let sender = sender::connect(transport::Full::new(), DC_ADDRESSES[0usize])
            .await
            .unwrap();

        // TODO Sender doesn't have a way to handle backpressure yet
        let (handle_tx, handle_rx) = mpsc::unbounded_channel();
        Self {
            sender,
            handle_tx,
            handle_rx,
        }
    }

    /// Return a new `ClientHandle` that can be used to invoke remote procedure calls.
    pub fn handle(&self) -> ClientHandle {
        ClientHandle {
            tx: self.handle_tx.clone(),
        }
    }

    /// Perform a single network step or processing of incoming requests via handles.
    pub async fn step(&mut self) -> Result<(), sender::ReadError> {
        let (result, request) = {
            let network = self.sender.step();
            let request = self.handle_rx.recv();
            pin_mut!(network);
            pin_mut!(request);
            match future::select(network, request).await {
                future::Either::Left((network, request)) => {
                    let request = request.now_or_never();
                    (Some(network), request)
                }
                future::Either::Right((request, network)) => {
                    let network = network.now_or_never();
                    (network, Some(request))
                }
            }
        };

        if let Some(request) = request {
            let request = request.expect("mpsc returned None");
            let response = self.sender.enqueue_body(request.request);
            drop(request.response.send(response));
        }

        // TODO request cancellation if this is Err
        // (perhaps a method on the sender to cancel_all)
        result.unwrap_or(Ok(()))
    }

    /// Run the client by repeatedly `step`ping the client until a graceful disconnection occurs,
    /// or a network error occurs.
    pub async fn run_until_disconnected(mut self) -> Result<(), sender::ReadError> {
        loop {
            self.step().await?;
        }
    }
}
