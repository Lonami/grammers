// Copyright 2020 - developers of the `grammers` project.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// https://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or https://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

//! Methods to deal with and offer access to updates.

use super::Client;
use crate::types::{ChatMap, Update};
pub use grammers_mtsender::{AuthorizationError, InvocationError};
pub use grammers_session::UpdateState;
use grammers_tl_types as tl;
use log::warn;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::time::sleep_until;

/// How long to wait after warning the user that the updates limit was exceeded.
const UPDATE_LIMIT_EXCEEDED_LOG_COOLDOWN: Duration = Duration::from_secs(300);

impl Client {
    /// Returns the next update from the buffer where they are queued until used.
    ///
    /// Similar using an iterator manually, this method will return `Some` until no more updates
    /// are available (e.g. a graceful disconnection occurred).
    ///
    /// # Examples
    ///
    /// ```
    /// # async fn f(mut client: grammers_client::Client) -> Result<(), Box<dyn std::error::Error>> {
    /// use grammers_client::Update;
    ///
    /// while let Some(update) = client.next_update().await? {
    ///     // Echo incoming messages and ignore everything else
    ///     match update {
    ///         Update::NewMessage(mut message) if !message.outgoing() => {
    ///             message.respond(message.text().into()).await?;
    ///         }
    ///         _ => {}
    ///     }
    /// }
    /// # Ok(())
    /// # }
    /// ```
    pub async fn next_update(&self) -> Result<Option<Update>, InvocationError> {
        loop {
            if let Some(updates) = self.0.updates.lock("client.next_update").pop_front() {
                return Ok(Some(updates));
            }

            let mut message_box = self.0.message_box.lock("client.next_update");
            if let Some(request) = message_box.get_difference() {
                drop(message_box);
                let response = self.invoke(&request).await?;
                let mut message_box = self.0.message_box.lock("client.next_update/get_difference");
                let mut chat_hashes = self.0.chat_hashes.lock("client.next_update/get_difference");
                let (updates, users, chats) =
                    message_box.apply_difference(response, &mut chat_hashes);

                self.extend_update_queue(updates, ChatMap::new(users, chats));
                continue;
            }

            if let Some(request) =
                message_box.get_channel_difference(&self.0.chat_hashes.lock("client.next_update"))
            {
                drop(message_box);
                let response = self.invoke(&request).await?;
                let mut message_box = self
                    .0
                    .message_box
                    .lock("client.next_update/get_channel_difference");

                let mut chat_hashes = self
                    .0
                    .chat_hashes
                    .lock("client.next_update/get_channel_difference");

                let (updates, users, chats) =
                    message_box.apply_channel_difference(request, response, &mut chat_hashes);

                self.extend_update_queue(updates, ChatMap::new(users, chats));
                continue;
            }

            let deadline = message_box.check_deadlines();
            drop(message_box);
            tokio::select! {
                _ = self.step() => {
                    log::trace!("stepped")
                }
                _ = sleep_until(deadline.into()) => {
                    log::trace!("slept")
                }
            }
        }
    }

    pub(crate) fn process_socket_updates(&self, all_updates: Vec<tl::enums::Updates>) {
        if all_updates.is_empty() {
            return;
        }

        let mut result = (Vec::new(), Vec::new(), Vec::new());
        let mut message_box = self.0.message_box.lock("client.process_socket_updates");
        let mut chat_hashes = self.0.chat_hashes.lock("client.process_socket_updates");

        for updates in all_updates {
            match message_box.process_updates(updates, &mut chat_hashes, &mut result.0) {
                Ok((users, chats)) => {
                    result.1.extend(users);
                    result.2.extend(chats);
                }
                Err(_) => return,
            }
        }

        let (updates, users, chats) = result;
        self.extend_update_queue(updates, ChatMap::new(users, chats));
    }

    fn extend_update_queue(&self, mut updates: Vec<tl::enums::Update>, chat_map: Arc<ChatMap>) {
        let mut guard = self.0.updates.lock("client.extend_update_queue");

        if let Some(limit) = self.0.config.params.update_queue_limit {
            if let Some(exceeds) = (guard.len() + updates.len()).checked_sub(limit + 1) {
                let exceeds = exceeds + 1;
                let now = Instant::now();
                let mut warn_guard = self
                    .0
                    .last_update_limit_warn
                    .lock("client.extend_update_queue");
                let notify = match *warn_guard {
                    None => true,
                    Some(instant) => now - instant > UPDATE_LIMIT_EXCEEDED_LOG_COOLDOWN,
                };

                updates.truncate(updates.len() - exceeds);
                if notify {
                    warn!(
                        "{} updates were dropped because the update_queue_limit was exceeded",
                        exceeds
                    );
                }

                *warn_guard = Some(now);
            }
        }

        guard.extend(
            updates
                .into_iter()
                .flat_map(|u| Update::new(self, u, &chat_map)),
        );
    }

    /// Synchronize the updates state to the session.
    pub fn sync_update_state(&self) {
        self.0.config.session.set_state(
            self.0
                .message_box
                .lock("client.sync_update_state")
                .session_state(),
        );
    }
}
