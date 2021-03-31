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
use std::fmt;
use std::net::SocketAddr;
use std::sync::{Arc, Mutex};
use tokio::sync::{mpsc, oneshot, Mutex as AsyncMutex};

/// When no locale is found, use this one instead.
const DEFAULT_LOCALE: &str = "en";

/// Configuration required to create a [`Client`] instance.
///
/// [`Client`]: struct.Client.html
pub struct Config {
    /// Session storage where data should persist, such as authorization key, server address,
    /// and other required information by the client.
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
    /// The threshold below which the library should automatically sleep on flood-wait and slow
    /// mode wait errors (inclusive). For instance, if an
    /// `RpcError { name: "FLOOD_WAIT", value: Some(17) }` (flood, must wait 17 seconds) occurs
    /// and `flood_sleep_threshold` is 20 (seconds), the library will `sleep` automatically for
    /// 17 seconds. If the error was for 21s, it would propagate the error instead instead.
    ///
    /// By default, the library will sleep on flood-waits below or equal to one minute (60
    /// seconds), but this can be disabled by passing `None`.
    ///
    /// On flood, the library will retry *once*. If the flood error occurs a second time after
    /// sleeping, the error will be returned.
    pub flood_sleep_threshold: Option<u32>,
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

pub(crate) struct ClientInner {
    // Used to implement `PartialEq`.
    pub(crate) id: i64,
    pub(crate) sender: AsyncMutex<Sender<transport::Full, mtp::Encrypted>>,
    pub(crate) dc_id: Mutex<i32>,
    // TODO try to avoid a mutex over the ENTIRE config; only the session needs it
    pub(crate) config: Mutex<Config>,
    pub(crate) handle_tx: mpsc::UnboundedSender<Request>,
    pub(crate) handle_rx: AsyncMutex<mpsc::UnboundedReceiver<Request>>,
    pub(crate) message_box: Mutex<MessageBox>,
    pub(crate) chat_hashes: ChatHashCache,
}

/// A client capable of connecting to Telegram and invoking requests.
///
/// This structure is the "entry point" of the library, from which you can start using the rest.
///
/// This structure owns all the necessary connections to Telegram, and has implementations for the
/// most basic methods, such as connecting, signing in, or processing network events.
///
/// On drop, all state is synchronized to the session. The [`FileSession`] attempts to save the
/// session to disk on drop as well, so everything should persist under normal operation.
///
/// [`FileSession`]: grammers_session::FileSession
#[derive(Clone)]
pub struct Client(pub(crate) Arc<ClientInner>);

/// A client handle which can be freely cloned and moved around tasks to invoke requests
/// concurrently.
///
/// This structure has implementations for most of the methods you will use, such as sending
/// messages, fetching users, answering bot callbacks, and so on.
#[derive(Clone)]
pub struct ClientHandle {
    // Used to implement `PartialEq`.
    pub(crate) id: i64,
    pub(crate) tx: mpsc::UnboundedSender<Request>,
    pub(crate) flood_sleep_threshold: Option<u32>,
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
            flood_sleep_threshold: Some(60),
        }
    }
}

// TODO move some stuff like drop into ClientInner?
impl Drop for Client {
    fn drop(&mut self) {
        self.sync_update_state();
    }
}

impl fmt::Debug for Client {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // TODO show more info, like user id and session name if present
        f.debug_struct("Client")
            .field("dc_id", &self.0.dc_id)
            .finish()
    }
}

impl fmt::Debug for ClientHandle {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // FIXME don't use dummy field, use finish_non_exhaustive once
        // https://github.com/rust-lang/rust/issues/67364 is closed
        f.debug_struct("ClientHandle").field("_", &"...").finish()
    }
}

impl PartialEq for Client {
    fn eq(&self, other: &Self) -> bool {
        self.0.id == other.0.id
    }
}

impl PartialEq for ClientHandle {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
    }
}
