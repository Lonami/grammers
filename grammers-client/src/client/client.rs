// Copyright 2020 - developers of the `grammers` project.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// https://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or https://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

use std::sync::Arc;

use grammers_mtsender::SenderPoolHandle;
use grammers_session::Session;

pub(crate) struct ClientInner {
    pub(crate) session: Arc<dyn Session>,
    pub(crate) api_id: i32,
    pub(crate) handle: SenderPoolHandle,
    pub(crate) configuration: ClientConfiguration,
    pub(crate) auth_copied_to_dcs: tokio::sync::Mutex<Vec<i32>>,
}

/// Wrapper around [`SenderPool`] to facilitate interaction with Telegram's API.
///
/// This structure is the "entry point" of the library, from which you can start using the rest.
///
/// This structure owns all the necessary connections to Telegram, and has implementations for the
/// most basic methods, such as connecting, signing in, or processing network events.
///
/// On drop, all state is synchronized to the session. The [`Session`] must be explicitly saved
/// to disk with its corresponding save method for persistence.
///
/// [`SenderPool`]: grammers_mtsender::SenderPool
/// [`Session`]: grammers_session::Session
#[derive(Clone)]
pub struct Client(pub(crate) Arc<ClientInner>);

use std::time::Duration;

/// Configuration that controls the [`Client`] behaviour when making requests.
pub struct ClientConfiguration {
    /// The retry policy to use when encountering errors after invoking a request.
    pub retry_policy: Box<dyn super::RetryPolicy>,

    /// Whether to call [`Session::cache_peer`] on all peer information that
    /// the high-level methods receive as a response (e.g. [`Client::iter_dialogs`]).
    ///
    /// The cached peers are then usable by other methods such as [`Client::resolve_peer`]
    /// for as long as the same persisted session is used.
    pub auto_cache_peers: bool,
}

/// Configuration that controls [`Client::stream_updates`].
pub struct UpdatesConfiguration {
    /// Should the client catch-up on updates sent to it while it was offline?
    ///
    /// By default, updates sent while the client was offline are ignored.
    pub catch_up: bool,

    /// How many updates may be buffered by the client at any given time.
    ///
    /// Telegram passively sends updates to the client through the open connection, so they must
    /// be buffered until the application has the capacity to consume them.
    ///
    /// Upon reaching this limit, updates will be dropped, and a warning log message will be
    /// emitted (but not too often, to avoid spamming the log), in order to let the developer
    /// know that they should either change how they handle updates or increase the limit.
    ///
    /// A limit of zero (`Some(0)`) indicates that updates should not be buffered.
    /// They will be immediately dropped, and no warning will ever be emitted.
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

impl Default for ClientConfiguration {
    /// Returns an instance that with an [`AutoSleep::default`] retry policy,
    /// where encountered peers are automatically passed to [`Session::cache_peer`].
    ///
    /// [`AutoSleep::default`]: super::AutoSleep::default
    fn default() -> Self {
        Self {
            retry_policy: Box::new(super::AutoSleep {
                threshold: Duration::from_secs(60),
                io_errors_as_flood_of: Some(Duration::from_secs(1)),
            }),
            auto_cache_peers: true,
        }
    }
}

impl Default for UpdatesConfiguration {
    /// Returns an instance that will not catch up, with a queue limit of 100 updates.
    fn default() -> Self {
        Self {
            catch_up: false,
            update_queue_limit: Some(100),
        }
    }
}
