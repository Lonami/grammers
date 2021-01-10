// Copyright 2020 - developers of the `grammers` project.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// https://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or https://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

//! Methods to deal with and offer access to updates.

use super::{Client, ClientHandle, Step};
use crate::types::{ChatMap, Update};
pub use grammers_mtsender::{AuthorizationError, InvocationError};
use grammers_session::Session;
pub use grammers_session::UpdateState;
use grammers_tl_types as tl;
use std::collections::VecDeque;
use std::sync::Arc;
use tokio::time::sleep_until;

pub struct UpdateIter {
    client: ClientHandle,
    updates: VecDeque<tl::enums::Update>,
    chat_hashes: Arc<ChatMap>,
}

impl UpdateIter {
    pub(crate) fn new(
        client: ClientHandle,
        updates: Vec<tl::enums::Update>,
        chat_hashes: Arc<ChatMap>,
    ) -> Self {
        Self {
            client,
            updates: updates.into(),
            chat_hashes,
        }
    }
}

impl Iterator for UpdateIter {
    type Item = Update;

    fn next(&mut self) -> Option<Self::Item> {
        while let Some(update) = self.updates.pop_front() {
            if let Some(update) = Update::new(&self.client, update, &self.chat_hashes) {
                return Some(update);
            }
        }

        None
    }
}

impl<S: Session> Client<S> {
    /// Returns an iterator with the last updates and some of the chats used in them
    /// in a map for easy access.
    ///
    /// Similar using an iterator manually, this method will return `Some` until no more updates
    /// are available (e.g. a disconnection occurred).
    pub async fn next_updates(&mut self) -> Result<Option<UpdateIter>, InvocationError> {
        loop {
            if let Some(request) = self.message_box.get_difference() {
                let response = self.invoke(&request).await?;
                let (updates, users, chats) = self.message_box.apply_difference(response);
                // > Implementations [have] to postpone updates received via the socket while
                // > filling gaps in the event and `Update` sequences, as well as avoid filling
                // > gaps in the same sequence.
                //
                // Basically, don't `step`, simply repeatedly get difference until we're done.
                return Ok(Some(UpdateIter::new(
                    self.handle(),
                    updates,
                    ChatMap::new(users, chats),
                )));
            }

            if let Some(request) = self.message_box.get_channel_difference(&self.chat_hashes) {
                let response = self.invoke(&request).await?;
                let (updates, users, chats) =
                    self.message_box.apply_channel_difference(request, response);
                return Ok(Some(UpdateIter::new(
                    self.handle(),
                    updates,
                    ChatMap::new(users, chats),
                )));
            }

            let deadline = self.message_box.timeout_deadline();
            tokio::select! {
                step = self.step() => {
                    match step? {
                        Step::Connected { updates } => if let Some(iter) = self.get_update_iter(updates) {
                            break Ok(Some(iter));
                        },
                        Step::Disconnected => break Ok(None),
                    }
                }
                _ = sleep_until(deadline) => {}
            }
        }
    }

    fn get_update_iter(&mut self, all_updates: Vec<tl::enums::Updates>) -> Option<UpdateIter> {
        if all_updates.is_empty() {
            return None;
        }

        let mut result = (Vec::new(), Vec::new(), Vec::new());
        for updates in all_updates {
            match self.message_box.process_updates(updates, &self.chat_hashes) {
                Ok(tuple) => {
                    result.0.extend(tuple.0);
                    result.1.extend(tuple.1);
                    result.2.extend(tuple.2);
                }
                Err(_) => return None,
            }
        }

        let (updates, users, chats) = result;
        Some(UpdateIter::new(
            self.handle(),
            updates,
            ChatMap::new(users, chats),
        ))
    }

    /// Synchronize the updates state to the session.
    pub fn sync_update_state(&mut self) {
        self.config
            .session
            .set_state(self.message_box.session_state())
    }
}
