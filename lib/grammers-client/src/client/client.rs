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
use std::net::SocketAddr;
use tokio::sync::{mpsc, oneshot};

/// When no locale is found, use this one instead.
const DEFAULT_LOCALE: &str = "en";

/// Configuration required to create a [`Client`] instance.
///
/// [`Client`]: struct.Client.html
pub struct Config<S: Session> {
    /// Session storage where data should persist, such as authorization key, server address,
    /// and other required information by the client.
    pub session: S,

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
    /// Should the client catch-up on updates sent to it while it was offline?
    ///
    /// By default, updates sent while the client was offline are ignored.
    // TODO catch up doesn't occur until we get an update that tells us if there was a gap, but
    // maybe we should forcibly try to get difference even if we didn't miss anything?
    pub catch_up: bool,
    /// Server address to connect to. By default, the library will connect to the address stored
    /// in the session file (or a default production address if no such address exists). This
    /// field can be used to override said address, and is most commonly used to connect to one
    /// of Telegram's test servers instead.
    pub server_addr: Option<SocketAddr>,
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
///
/// On drop, all state is synchronized to the session. The [`FileSession`] attempts to save the
/// session to disk on drop as well, so everything should persist under normal operation.
///
/// [`FileSession`]: grammers_session::FileSession
pub struct Client<S: Session> {
    pub(crate) sender: Sender<transport::Full, mtp::Encrypted>,
    pub(crate) dc_id: i32,
    pub(crate) config: Config<S>,
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

        let mut system_lang_code = String::new();
        let mut lang_code = String::new();

        #[cfg(not(target_os = "android"))]
        {
            system_lang_code.push_str(&locate_locale::system());
            lang_code.push_str(&locate_locale::user());
        }
        if system_lang_code.is_empty() {
            system_lang_code.push_str(DEFAULT_LOCALE);
        }
        if lang_code.is_empty() {
            lang_code.push_str(DEFAULT_LOCALE);
        }

        Self {
            device_model: format!("{} {}", info.os_type(), info.bitness()),
            system_version: info.version().to_string(),
            app_version: env!("CARGO_PKG_VERSION").to_string(),
            system_lang_code,
            lang_code,
            catch_up: false,
            server_addr: None,
        }
    }
}

impl<S: Session> Drop for Client<S> {
    fn drop(&mut self) {
        self.sync_update_state();
    }
}
