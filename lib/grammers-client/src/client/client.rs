// Copyright 2020 - developers of the `grammers` project.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// https://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or https://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.
use crate::utils::{AsyncMutex, Mutex};
use grammers_mtproto::{mtp, transport};
use grammers_mtsender::{Enqueuer, Sender};
use grammers_session::{ChatHashCache, MessageBox, Session};
use std::collections::VecDeque;
use std::fmt;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Instant;
use tokio::sync::Notify;

/// When no locale is found, use this one instead.
const DEFAULT_LOCALE: &str = "en";

/// Configuration required to create a [`Client`] instance.
///
/// [`Client`]: struct.Client.html
pub struct Config {
    /// Session storage where data should persist, such as authorization key, server address,
    /// and other required information by the client.
    pub session: Session,

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
    /// How many updates may be buffered by the client at any given time.
    ///
    /// Telegram passively sends updates to the client through the open connection, so they must
    /// be buffered until the application has the capacity to consume them.
    ///
    /// Upon reaching this limit, updates will be dropped, and a warning log message will be
    /// emitted (but not too often, to avoid spamming the log), in order to let the developer
    /// know that they should either change how they handle updates or increase the limit.
    ///
    /// A limit of zero (`0`) indicates that updates should not be buffered. They will be
    /// immediately dropped, and no warning will ever be emitted.
    ///
    /// A limit of `None` disables the upper bound for the buffer. This is not recommended, as it
    /// could eventually lead to memory exhaustion. This option will also not emit any warnings.
    ///
    /// The default limit, which may change at any time, should be enough for user accounts,
    /// although bot accounts may need to increase the limit depending on their capacity.
    ///
    /// When the limit is `Some`, a buffer to hold that many updates will be pre-allocated.
    pub update_queue_limit: Option<usize>,
}

pub(crate) struct ClientInner {
    // Used to implement `PartialEq`.
    pub(crate) id: i64,
    pub(crate) sender: AsyncMutex<Sender<transport::Full, mtp::Encrypted>>,
    pub(crate) stepping_done: Notify,
    pub(crate) dc_id: Mutex<i32>,
    pub(crate) config: Config,
    pub(crate) message_box: Mutex<MessageBox>,
    pub(crate) chat_hashes: Mutex<ChatHashCache>,
    // When did we last warn the user that the update queue filled up?
    // This is used to avoid spamming the log.
    pub(crate) last_update_limit_warn: Mutex<Option<Instant>>,
    pub(crate) updates: Mutex<VecDeque<crate::types::Update>>,
    // Used to avoid locking the entire sender when enqueueing requests.
    pub(crate) request_tx: Mutex<Enqueuer>,
}

/// A client capable of connecting to Telegram and invoking requests.
///
/// This structure is the "entry point" of the library, from which you can start using the rest.
///
/// This structure owns all the necessary connections to Telegram, and has implementations for the
/// most basic methods, such as connecting, signing in, or processing network events.
///
/// On drop, all state is synchronized to the session. The [`Session`] must be explicitly saved
/// to disk with [`Session::save_to_file`] for persistence
///
/// [`Session`]: grammers_session::Session
#[derive(Clone)]
pub struct Client(pub(crate) Arc<ClientInner>);

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
            update_queue_limit: Some(100),
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

impl PartialEq for Client {
    fn eq(&self, other: &Self) -> bool {
        self.0.id == other.0.id
    }
}
