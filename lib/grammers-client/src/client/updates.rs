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
use std::sync::Arc;
use tokio::time::sleep_until;

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
    ///         Update::NewMessage(message) if !message.outgoing() => {
    ///             client.send_message(&chat, message.text().into()).await?;
    ///         }
    ///         _ => {}
    ///     }
    /// }
    /// # Ok(())
    /// # }
    /// ```
    pub async fn next_update(&self) -> Result<Option<Update>, InvocationError> {
        loop {
            if let Some(updates) = self.0.updates.lock().unwrap().pop_front() {
                return Ok(Some(updates));
            }

            let mut message_box = self.0.message_box.lock().unwrap();
            if let Some(request) = message_box.get_difference() {
                drop(message_box);
                let response = self.invoke(&request).await?;
                let mut message_box = self.0.message_box.lock().unwrap();
                let (updates, users, chats) = message_box.apply_difference(response);
                // > Implementations [have] to postpone updates received via the socket while
                // > filling gaps in the event and `Update` sequences, as well as avoid filling
                // > gaps in the same sequence.
                //
                // Basically, don't `step`, simply repeatedly get difference until we're done.
                // TODO ^ but that's wrong because invoke necessarily steps
                self.extend_update_queue(updates, ChatMap::new(users, chats));
                continue;
            }

            if let Some(request) = message_box.get_channel_difference(&self.0.chat_hashes) {
                drop(message_box);
                let response = self.invoke(&request).await?;
                let mut message_box = self.0.message_box.lock().unwrap();
                let (updates, users, chats) =
                    message_box.apply_channel_difference(request, response);

                self.extend_update_queue(updates, ChatMap::new(users, chats));
                continue;
            }

            let deadline = message_box.timeout_deadline();
            drop(message_box);
            tokio::select! {
                _ = self.step() => {}
                _ = sleep_until(deadline.into()) => {}
            }
        }
    }

    pub(crate) fn process_socket_updates(&self, all_updates: Vec<tl::enums::Updates>) {
        if all_updates.is_empty() {
            return;
        }

        let mut result = (Vec::new(), Vec::new(), Vec::new());
        let mut message_box = self.0.message_box.lock().unwrap();
        for updates in all_updates {
            match message_box.process_updates(updates, &self.0.chat_hashes) {
                Ok(tuple) => {
                    result.0.extend(tuple.0);
                    result.1.extend(tuple.1);
                    result.2.extend(tuple.2);
                }
                Err(_) => return,
            }
        }

        let (updates, users, chats) = result;
        self.extend_update_queue(updates, ChatMap::new(users, chats));
    }

    fn extend_update_queue(&self, updates: Vec<tl::enums::Update>, chat_map: Arc<ChatMap>) {
        self.0.updates.lock().unwrap().extend(
            updates
                .into_iter()
                .flat_map(|u| Update::new(self, u, &chat_map)),
        );
    }

    /// Synchronize the updates state to the session.
    pub fn sync_update_state(&self) {
        self.0
            .config
            .session
            .set_state(self.0.message_box.lock().unwrap().session_state());
    }
}
