// Copyright 2020 - developers of the `grammers` project.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// https://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or https://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.
pub use super::updates::UpdateIter;
use crate::types::{ChatHashCache, MessageBox};
use grammers_mtproto::{mtp, transport};
use grammers_mtsender::{InvocationError, Sender};
use grammers_session::Session;
use grammers_tl_types as tl;
use tokio::sync::{mpsc, oneshot};

/// When no locale is found, use this one instead.
const DEFAULT_LOCALE: &str = "en";

/// Configuration required to create a [`Client`] instance.
///
/// [`Client`]: struct.Client.html
pub struct Config {
    /// Session storage where data should persist, such as authorization key, server address,
    /// and other required information by the client.
    // Using `Box<dyn ...>` and not a type parameter for simplicity.
    // Access to the session is uncommon, so it's unlikely to affect performance.
    pub session: Box<dyn Session>,

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

/// Request messages that the `ClientHandle` uses to communicate with the `Client`.
pub(crate) enum Request {
    Rpc {
        request: Vec<u8>,
        response: oneshot::Sender<oneshot::Receiver<Result<Vec<u8>, InvocationError>>>,
    },
    Disconnect {
        response: oneshot::Sender<()>,
    },
}

/// A client capable of connecting to Telegram and invoking requests.
///
/// This structure is the "entry point" of the library, from which you can start using the rest.
///
/// This structure owns all the necessary connections to Telegram, and has implementations for the
/// most basic methods, such as connecting, signing in, or processing network events.
///
/// To invoke multiple requests concurrently, [`ClientHandle`] must be used instead, and this
/// structure will coordinate all of them.
pub struct Client {
    pub(crate) sender: Sender<transport::Full, mtp::Encrypted>,
    /// Data center ID for the main sender.
    pub(crate) dc_id: i32,
    pub(crate) config: Config,
    pub(crate) handle_tx: mpsc::UnboundedSender<Request>,
    pub(crate) handle_rx: mpsc::UnboundedReceiver<Request>,
    pub(crate) message_box: MessageBox,
    pub(crate) chat_hashes: ChatHashCache,
}

/// A client handle which can be freely cloned and moved around tasks to invoke requests
/// concurrently.
///
/// This structure has implementations for most of the methods you will use, such as sending
/// messages, fetching users, answering bot callbacks, and so on.
#[derive(Clone)]
pub struct ClientHandle {
    pub(crate) tx: mpsc::UnboundedSender<Request>,
}

/// A network step.
pub enum Step {
    /// The `Client` is still connected, and a possibly-empty list of updates were received
    /// during this step.
    Connected { updates: Vec<tl::enums::Updates> },
    /// The `Client` has been gracefully disconnected, and no more calls to `step` are needed.
    Disconnected,
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

impl Drop for Client {
    fn drop(&mut self) {
        self.sync_update_state();
    }
}
