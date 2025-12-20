// Copyright 2020 - developers of the `grammers` project.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// https://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or https://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

//! Methods to deal with and offer access to updates.

#![allow(deprecated)]

use super::{Client, UpdatesConfiguration};
use crate::types::{PeerMap, Update};
use grammers_mtsender::InvocationError;
use grammers_session::Session;
use grammers_session::types::{PeerId, PeerInfo, UpdateState, UpdatesState};
pub use grammers_session::updates::{MessageBoxes, PrematureEndReason, State, UpdatesLike};
use grammers_tl_types as tl;
use log::{trace, warn};
use std::collections::VecDeque;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::mpsc;
use tokio::time::timeout_at;

/// How long to wait after warning the user that the updates limit was exceeded.
const UPDATE_LIMIT_EXCEEDED_LOG_COOLDOWN: Duration = Duration::from_secs(300);

// See https://core.telegram.org/method/updates.getChannelDifference.
const BOT_CHANNEL_DIFF_LIMIT: i32 = 100000;
const USER_CHANNEL_DIFF_LIMIT: i32 = 100;

fn prepare_channel_difference(
    mut request: tl::functions::updates::GetChannelDifference,
    session: &dyn Session,
    message_box: &mut MessageBoxes,
) -> Option<tl::functions::updates::GetChannelDifference> {
    let id = match &request.channel {
        tl::enums::InputChannel::Channel(channel) => PeerId::channel(channel.channel_id),
        _ => unreachable!(),
    };

    if let Some(PeerInfo::Channel {
        id,
        auth: Some(auth),
        ..
    }) = session.peer(id)
    {
        request.channel = tl::enums::InputChannel::Channel(tl::types::InputChannel {
            channel_id: id,
            access_hash: auth.hash(),
        });
        request.limit = if session
            .peer(PeerId::self_user())
            .map(|user| match user {
                PeerInfo::User { bot, .. } => bot.unwrap_or(false),
                _ => false,
            })
            .unwrap_or(false)
        {
            BOT_CHANNEL_DIFF_LIMIT
        } else {
            USER_CHANNEL_DIFF_LIMIT
        };
        trace!("requesting {:?}", request);
        Some(request)
    } else {
        warn!(
            "cannot getChannelDifference for {:?} as we're missing its hash",
            id
        );
        message_box.end_channel_difference(PrematureEndReason::Banned);
        None
    }
}

pub struct UpdateStream {
    client: Client,
    message_box: MessageBoxes,
    // When did we last warn the user that the update queue filled up?
    // This is used to avoid spamming the log.
    last_update_limit_warn: Option<Instant>,
    buffer: VecDeque<(tl::enums::Update, State, Arc<crate::types::PeerMap>)>,
    updates: mpsc::UnboundedReceiver<UpdatesLike>,
    configuration: UpdatesConfiguration,
    should_get_state: bool,
}

impl UpdateStream {
    pub async fn next(&mut self) -> Result<Update, InvocationError> {
        let (update, state, peers) = self.next_raw().await?;
        Ok(Update::new(&self.client, update, state, &peers))
    }

    pub async fn next_raw(
        &mut self,
    ) -> Result<(tl::enums::Update, State, Arc<PeerMap>), InvocationError> {
        if self.should_get_state {
            self.should_get_state = false;
            match self
                .client
                .invoke(&tl::functions::updates::GetState {})
                .await
            {
                Ok(tl::enums::updates::State::State(state)) => {
                    self.client
                        .0
                        .session
                        .set_update_state(UpdateState::All(UpdatesState {
                            pts: state.pts,
                            qts: state.qts,
                            date: state.date,
                            seq: state.seq,
                            channels: Vec::new(),
                        }));
                }
                Err(_err) => {
                    // The account may no longer actually be logged in, or it can rarely fail.
                    // `message_box` will try to correct its state as updates arrive.
                }
            }
        }

        loop {
            let (deadline, get_diff, get_channel_diff) = {
                if let Some(update) = self.buffer.pop_front() {
                    return Ok(update);
                }
                (
                    self.message_box.check_deadlines(), // first, as it might trigger differences
                    self.message_box.get_difference(),
                    self.message_box.get_channel_difference().and_then(|gd| {
                        prepare_channel_difference(
                            gd,
                            self.client.0.session.as_ref(),
                            &mut self.message_box,
                        )
                    }),
                )
            };

            if let Some(request) = get_diff {
                let response = self.client.invoke(&request).await?;
                let (updates, users, chats) = self.message_box.apply_difference(response);
                let peers = PeerMap::new(users, chats);
                self.client.cache_peers_maybe(&peers);
                self.extend_update_queue(updates, peers);
                continue;
            }

            if let Some(request) = get_channel_diff {
                let maybe_response = self.client.invoke(&request).await;

                let response = match maybe_response {
                    Ok(r) => r,
                    Err(e) if e.is("PERSISTENT_TIMESTAMP_OUTDATED") => {
                        // According to Telegram's docs:
                        // "Channel internal replication issues, try again later (treat this like an RPC_CALL_FAIL)."
                        // We can treat this as "empty difference" and not update the local pts.
                        // Then this same call will be retried when another gap is detected or timeout expires.
                        //
                        // Another option would be to literally treat this like an RPC_CALL_FAIL and retry after a few
                        // seconds, but if Telegram is having issues it's probably best to wait for it to send another
                        // update (hinting it may be okay now) and retry then.
                        //
                        // This is a bit hacky because MessageBox doesn't really have a way to "not update" the pts.
                        // Instead we manually extract the previously-known pts and use that.
                        log::warn!(
                            "Getting difference for channel updates caused PersistentTimestampOutdated; ending getting difference prematurely until server issues are resolved"
                        );
                        {
                            self.message_box
                                .end_channel_difference(PrematureEndReason::TemporaryServerIssues);
                        }
                        continue;
                    }
                    Err(e) if e.is("CHANNEL_PRIVATE") => {
                        log::info!(
                            "Account is now banned so we can no longer fetch updates with request: {:?}",
                            request
                        );
                        {
                            self.message_box
                                .end_channel_difference(PrematureEndReason::Banned);
                        }
                        continue;
                    }
                    Err(InvocationError::Rpc(rpc_error)) if rpc_error.code == 500 => {
                        log::warn!("Telegram is having internal issues: {:#?}", rpc_error);
                        {
                            self.message_box
                                .end_channel_difference(PrematureEndReason::TemporaryServerIssues);
                        }
                        continue;
                    }
                    Err(e) => return Err(e),
                };

                let (updates, users, chats) = self.message_box.apply_channel_difference(response);

                let peers = PeerMap::new(users, chats);
                self.client.cache_peers_maybe(&peers);
                self.extend_update_queue(updates, peers);
                continue;
            }

            match timeout_at(deadline.into(), self.updates.recv()).await {
                Ok(Some(updates)) => self.process_socket_updates(updates),
                Ok(None) => break Err(InvocationError::Dropped),
                Err(_) => {}
            }
        }
    }

    pub(crate) fn process_socket_updates(&mut self, updates: UpdatesLike) {
        let mut result = Option::<(Vec<_>, Vec<_>, Vec<_>)>::None;
        match self.message_box.process_updates(updates) {
            Ok(tup) => {
                if let Some(res) = result.as_mut() {
                    res.0.extend(tup.0);
                    res.1.extend(tup.1);
                    res.2.extend(tup.2);
                } else {
                    result = Some(tup);
                }
            }
            Err(_) => return,
        }

        if let Some((updates, users, chats)) = result {
            let peers = PeerMap::new(users, chats);
            self.client.cache_peers_maybe(&peers);
            self.extend_update_queue(updates, peers);
        }
    }

    fn extend_update_queue(
        &mut self,
        mut updates: Vec<(tl::enums::Update, State)>,
        peer_map: Arc<PeerMap>,
    ) {
        if let Some(limit) = self.configuration.update_queue_limit {
            if let Some(exceeds) = (self.buffer.len() + updates.len()).checked_sub(limit + 1) {
                let exceeds = exceeds + 1;
                let now = Instant::now();
                let notify = match self.last_update_limit_warn {
                    None => true,
                    Some(instant) => now - instant > UPDATE_LIMIT_EXCEEDED_LOG_COOLDOWN,
                };

                updates.truncate(updates.len() - exceeds);
                if notify {
                    log::warn!(
                        "{} updates were dropped because the update_queue_limit was exceeded",
                        exceeds
                    );
                }

                self.last_update_limit_warn = Some(now);
            }
        }

        self.buffer
            .extend(updates.into_iter().map(|(u, s)| (u, s, peer_map.clone())));
    }

    /// Synchronize the updates state to the session.
    pub fn sync_update_state(&self) {
        self.client
            .0
            .session
            .set_update_state(UpdateState::All(self.message_box.session_state()));
    }
}

impl Drop for UpdateStream {
    fn drop(&mut self) {
        self.sync_update_state();
    }
}

impl Client {
    /// Returns an asynchronous stream of processed updates.
    ///
    /// The updates are guaranteed to be in order, and any gaps will be resolved.\
    /// **Important** to note that for gaps to be resolved, the peers must have been
    /// persisted in the session cache beforehand (i.e. be retrievable with [`Session::peer`]).
    /// A good way to achieve this is to use [`Self::iter_dialogs`] at least once after login.
    ///
    /// The updates are wrapped in [`crate::Update`] to make them more convenient to use,
    /// but their raw type is still accessible to bridge any missing functionality.
    pub fn stream_updates(
        &self,
        updates: mpsc::UnboundedReceiver<UpdatesLike>,
        configuration: UpdatesConfiguration,
    ) -> UpdateStream {
        let message_box = if configuration.catch_up {
            MessageBoxes::load(self.0.session.updates_state())
        } else {
            // If the user doesn't want to bother with catching up on previous update, start with
            // pristine state instead.
            MessageBoxes::new()
        };
        // Don't bother getting pristine update state if we're not logged in.
        let should_get_state =
            message_box.is_empty() && self.0.session.peer(PeerId::self_user()).is_some();

        UpdateStream {
            client: self.clone(),
            message_box,
            last_update_limit_warn: None,
            buffer: VecDeque::new(),
            updates,
            configuration,
            should_get_state,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use core::future::Future;

    fn get_update_stream() -> UpdateStream {
        panic!()
    }

    #[test]
    fn ensure_next_update_future_impls_send() {
        if false {
            // We just want it to type-check, not actually run.
            fn typeck(_: impl Future + Send) {}
            typeck(get_update_stream().next());
        }
    }
}
