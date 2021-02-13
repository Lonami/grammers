// Copyright 2020 - developers of the `grammers` project.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// https://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or https://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.
mod adaptor;
mod defs;

use super::ChatHashCache;
pub(crate) use defs::{Entry, Gap, MessageBox};
use defs::{PtsInfo, NO_SEQ, POSSIBLE_GAP_TIMEOUT};
pub use grammers_session::UpdateState;
use grammers_tl_types as tl;
use log::{debug, info, trace};
use std::cmp::Ordering;
use std::collections::{HashMap, HashSet};
use tokio::time::Instant;

fn next_updates_deadline() -> Instant {
    Instant::now() + defs::NO_UPDATES_TIMEOUT
}

/// Creation, querying, and setting base state.
impl MessageBox {
    pub(crate) fn new() -> Self {
        Self {
            getting_diff: false,
            getting_channel_diff: HashSet::new(),
            deadline: next_updates_deadline(),
            date: 1,
            seq: 0,
            pts_map: HashMap::new(),
            possible_gap: HashMap::new(),
            possible_gap_deadline: None,
        }
    }

    pub(crate) fn load(state: UpdateState) -> Self {
        let mut pts_map = HashMap::with_capacity(2 + state.channels.len());
        pts_map.insert(Entry::AccountWide, state.pts);
        pts_map.insert(Entry::SecretChats, state.qts);
        pts_map.extend(
            state
                .channels
                .iter()
                .map(|(id, pts)| (Entry::Channel(*id), *pts)),
        );

        MessageBox {
            getting_diff: false,
            getting_channel_diff: HashSet::new(),
            deadline: next_updates_deadline(),
            date: state.date,
            seq: state.seq,
            pts_map,
            possible_gap: HashMap::new(),
            possible_gap_deadline: None,
        }
    }

    /// Return the current state in a format that sessions understand.
    pub(crate) fn session_state(&self) -> UpdateState {
        UpdateState {
            pts: *self.pts_map.get(&Entry::AccountWide).unwrap_or(&0),
            qts: *self.pts_map.get(&Entry::SecretChats).unwrap_or(&0),
            date: self.date,
            seq: self.seq,
            channels: self
                .pts_map
                .iter()
                .filter_map(|(key, pts)| match key {
                    Entry::Channel(id) => Some((*id, *pts)),
                    _ => None,
                })
                .collect(),
        }
    }

    /// Return true if the message box is empty and has no state yet.
    pub(crate) fn is_empty(&self) -> bool {
        *self.pts_map.get(&Entry::AccountWide).unwrap_or(&NO_SEQ) == NO_SEQ
    }

    /// Return the next deadline when receiving updates should timeout.
    ///
    /// When this deadline is met, it means that get difference needs to be called.
    pub(crate) fn timeout_deadline(&self) -> Instant {
        self.possible_gap_deadline.unwrap_or(self.deadline)
    }

    // Note: calling this method is **really** important, or we'll start fetching updates from
    // scratch.
    pub(crate) fn set_state(&mut self, state: tl::enums::updates::State) {
        let state: tl::types::updates::State = state.into();
        self.date = state.date;
        self.seq = state.seq;
        self.pts_map.insert(Entry::AccountWide, state.pts);
        self.pts_map.insert(Entry::SecretChats, state.qts);
    }
}

// "Normal" updates flow (processing and detection of gaps).
impl MessageBox {
    /// Process an update and return what should be done with it.
    pub(crate) fn process_updates(
        &mut self,
        updates: tl::enums::Updates,
        chat_hashes: &ChatHashCache,
    ) -> Result<
        (
            Vec<tl::enums::Update>,
            Vec<tl::enums::User>,
            Vec<tl::enums::Chat>,
        ),
        Gap,
    > {
        self.deadline = next_updates_deadline();

        // Top level, when handling received `updates` and `updatesCombined`.
        // `updatesCombined` groups all the fields we care about, which is why we use it.
        let tl::types::UpdatesCombined {
            date,
            seq_start,
            seq,
            updates,
            users,
            chats,
        } = match adaptor::adapt(updates, chat_hashes) {
            Ok(combined) => combined,
            Err(Gap) => {
                self.getting_diff = true;
                return Err(Gap);
            }
        };

        // > For all the other [not `updates` or `updatesCombined`] `Updates` type constructors
        // > there is no need to check `seq` or change a local state.
        if seq_start != NO_SEQ {
            match (self.seq + 1).cmp(&seq_start) {
                // Apply
                Ordering::Equal => {}
                // Ignore
                Ordering::Greater => {
                    debug!(
                        "skipping updates that were already handled at seq = {}",
                        self.seq
                    );
                    return Ok((Vec::new(), users, chats));
                }
                Ordering::Less => {
                    debug!(
                        "gap detected (local seq {}, remote seq {})",
                        self.seq, seq_start
                    );
                    self.getting_diff = true;
                    return Err(Gap);
                }
            }

            self.date = date;
            if seq != NO_SEQ {
                self.seq = seq;
                trace!("updated date = {}, seq = {}", date, seq);
            }
        }

        let mut result = updates
            .into_iter()
            .filter_map(|u| self.apply_pts_info(u))
            .collect::<Vec<_>>();

        // For each update in possible gaps, see if the gap has been resolved already.
        // Borrow checker doesn't know that `possible_gap` won't be changed by `apply_pts_info`.
        let keys = self.possible_gap.keys().copied().collect::<Vec<_>>();
        for key in keys {
            for _ in 0..self.possible_gap.get(&key).unwrap().len() {
                let update = self.possible_gap.get_mut(&key).unwrap().remove(0);
                // If this fails to apply, it will get re-inserted at the end.
                // All should fail, so the order will be preserved (it would've cycled once).
                if let Some(update) = self.apply_pts_info(update) {
                    result.push(update);
                }
            }
        }

        // Clear now-empty gaps. If all are cleared, also clear the gap deadline.
        self.possible_gap.retain(|_, v| !v.is_empty());
        if self.possible_gap.is_empty() {
            debug!("successfully resolved gap by waiting");
            self.possible_gap_deadline = None;
        }

        Ok((result, users, chats))
    }

    fn apply_pts_info(&mut self, update: tl::enums::Update) -> Option<tl::enums::Update> {
        let pts = match PtsInfo::from_update(&update) {
            Some(pts) => pts,
            None => return Some(update),
        };

        let local_pts = if let Some(&local_pts) = self.pts_map.get(&pts.entry) {
            match (local_pts + pts.pts_count).cmp(&pts.pts) {
                // Apply
                Ordering::Equal => {
                    trace!(
                        "applying update for {:?} (local {:?}, count {:?}, remote {:?})",
                        pts.entry,
                        local_pts,
                        pts.pts_count,
                        pts.pts
                    );
                    local_pts
                }
                // Ignore
                Ordering::Greater => {
                    debug!(
                        "skipping update for {:?} (local {:?}, count {:?}, remote {:?})",
                        pts.entry, local_pts, pts.pts_count, pts.pts
                    );
                    return None;
                }
                Ordering::Less => {
                    info!(
                        "gap on update for {:?} (local {:?}, count {:?}, remote {:?})",
                        pts.entry, local_pts, pts.pts_count, pts.pts
                    );
                    // TODO store chats too?
                    self.possible_gap.entry(pts.entry).or_default().push(update);
                    if self.possible_gap_deadline.is_none() {
                        self.possible_gap_deadline = Some(Instant::now() + POSSIBLE_GAP_TIMEOUT);
                    }
                    return None;
                }
            }
        } else {
            // No previous `pts` known, and because this update has to be "right" (it's the first one) our
            // `local_pts` must be one less.
            pts.pts - 1
        };

        // For example, when we're in a channel, we immediately receive:
        // * ReadChannelInbox (pts = X)
        // * NewChannelMessage (pts = X, pts_count = 1)
        //
        // Notice how both `pts` are the same. If we stored the one from the first, then the second one would
        // be considered "already handled" and ignored, which is not desirable. Instead, advance local `pts`
        // by `pts_count` (which is 0 for updates not directly related to messages, like reading inbox).
        self.pts_map.insert(pts.entry, local_pts + pts.pts_count);
        Some(update)
    }
}

/// Getting and applying account difference.
impl MessageBox {
    /// Return the request that needs to be made to get the difference, if any.
    pub(crate) fn get_difference(&mut self) -> Option<tl::functions::updates::GetDifference> {
        if self.getting_diff || Instant::now() > self.possible_gap_deadline.unwrap_or(self.deadline)
        {
            if self.possible_gap_deadline.is_some() {
                info!("gap was not resolved after waiting");
                self.getting_diff = true;
                self.possible_gap_deadline = None;
                // TODO shouldn't this do getChannelDifference?
                self.possible_gap.clear();
            }

            Some(tl::functions::updates::GetDifference {
                pts: self.pts_map.get(&Entry::AccountWide).copied().unwrap_or(1),
                pts_total_limit: None,
                date: self.date,
                qts: self.pts_map.get(&Entry::SecretChats).copied().unwrap_or(1),
            })
        } else {
            None
        }
    }

    pub(crate) fn apply_difference(
        &mut self,
        difference: tl::enums::updates::Difference,
    ) -> (
        Vec<tl::enums::Update>,
        Vec<tl::enums::User>,
        Vec<tl::enums::Chat>,
    ) {
        self.deadline = next_updates_deadline();

        match difference {
            tl::enums::updates::Difference::Empty(diff) => {
                debug!(
                    "handling empty difference (date = {}, seq = {}); no longer getting diff",
                    diff.date, diff.seq
                );
                self.date = diff.date;
                self.seq = diff.seq;
                self.getting_diff = false;
                (Vec::new(), Vec::new(), Vec::new())
            }
            tl::enums::updates::Difference::Difference(diff) => {
                debug!(
                    "handling full difference {:?}; no longer getting diff",
                    diff.state
                );
                self.getting_diff = false;
                self.apply_difference_type(diff)
            }
            tl::enums::updates::Difference::Slice(tl::types::updates::DifferenceSlice {
                new_messages,
                new_encrypted_messages,
                other_updates,
                chats,
                users,
                intermediate_state: state,
            }) => {
                debug!("handling partial difference {:?}", state);
                self.apply_difference_type(tl::types::updates::Difference {
                    new_messages,
                    new_encrypted_messages,
                    other_updates,
                    chats,
                    users,
                    state,
                })
            }
            tl::enums::updates::Difference::TooLong(diff) => {
                debug!(
                    "handling too-long difference (pts = {}); no longer getting diff",
                    diff.pts
                );
                self.pts_map.insert(Entry::AccountWide, diff.pts);
                self.getting_diff = false;
                (Vec::new(), Vec::new(), Vec::new())
            }
        }
    }

    fn apply_difference_type(
        &mut self,
        tl::types::updates::Difference {
            new_messages,
            new_encrypted_messages,
            other_updates: mut updates,
            chats,
            users,
            state: tl::enums::updates::State::State(state),
        }: tl::types::updates::Difference,
    ) -> (
        Vec<tl::enums::Update>,
        Vec<tl::enums::User>,
        Vec<tl::enums::Chat>,
    ) {
        self.pts_map.insert(Entry::AccountWide, state.pts);
        self.pts_map.insert(Entry::SecretChats, state.qts);
        self.date = state.date;
        self.seq = state.seq;

        updates.iter().for_each(|u| match u {
            tl::enums::Update::ChannelTooLong(c) => {
                // `c.pts`, if any, is the channel's current `pts`; we do not need this.
                self.getting_channel_diff.insert(c.channel_id);
            }
            _ => {}
        });

        updates.extend(
            new_messages
                .into_iter()
                .map(|message| {
                    tl::types::UpdateNewMessage {
                        message,
                        pts: NO_SEQ,
                        pts_count: NO_SEQ,
                    }
                    .into()
                })
                .chain(new_encrypted_messages.into_iter().map(|message| {
                    tl::types::UpdateNewEncryptedMessage {
                        message,
                        qts: NO_SEQ,
                    }
                    .into()
                })),
        );

        (updates, users, chats)
    }
}

/// Getting and applying channel difference.
impl MessageBox {
    /// Return the request that needs to be made to get a channel's difference, if any.
    pub(crate) fn get_channel_difference(
        &mut self,
        chat_hashes: &ChatHashCache,
    ) -> Option<tl::functions::updates::GetChannelDifference> {
        let channel_id = *self.getting_channel_diff.iter().next()?;
        let channel = if let Some(channel) = chat_hashes.get_input_channel(channel_id) {
            channel
        } else {
            self.getting_channel_diff.remove(&channel_id);
            return None;
        };

        if let Some(&pts) = self.pts_map.get(&Entry::Channel(channel_id)) {
            Some(tl::functions::updates::GetChannelDifference {
                force: false,
                channel,
                filter: tl::enums::ChannelMessagesFilter::Empty,
                pts,
                limit: if chat_hashes.is_self_bot() {
                    defs::BOT_CHANNEL_DIFF_LIMIT
                } else {
                    defs::USER_CHANNEL_DIFF_LIMIT
                },
            })
        } else {
            self.getting_channel_diff.remove(&channel_id);
            None
        }
    }

    pub(crate) fn apply_channel_difference(
        &mut self,
        request: tl::functions::updates::GetChannelDifference,
        difference: tl::enums::updates::ChannelDifference,
    ) -> (
        Vec<tl::enums::Update>,
        Vec<tl::enums::User>,
        Vec<tl::enums::Chat>,
    ) {
        self.deadline = next_updates_deadline();

        let channel_id = match request.channel {
            tl::enums::InputChannel::Channel(c) => c.channel_id,
            _ => panic!("request had wrong input channel"),
        };

        // TODO refetch updates after timeout
        match difference {
            tl::enums::updates::ChannelDifference::Empty(diff) => {
                assert!(!diff.r#final);
                debug!(
                    "handling empty channel {} difference (pts = {}); no longer getting diff",
                    channel_id, diff.pts
                );
                self.getting_channel_diff.remove(&channel_id);
                self.pts_map.insert(Entry::Channel(channel_id), diff.pts);
                (Vec::new(), Vec::new(), Vec::new())
            }
            tl::enums::updates::ChannelDifference::TooLong(diff) => {
                assert!(!diff.r#final);
                info!(
                    "handling too long channel {} difference; no longer getting diff",
                    channel_id
                );
                match diff.dialog {
                    tl::enums::Dialog::Dialog(d) => {
                        self.pts_map.insert(
                            Entry::Channel(channel_id),
                            d.pts.expect(
                                "channelDifferenceTooLong dialog did not actually contain a pts",
                            ),
                        );
                    }
                    tl::enums::Dialog::Folder(_) => {
                        panic!("received a folder on channelDifferenceTooLong")
                    }
                }
                // This `diff` has the "latest messages and corresponding chats", but it would
                // be strange to give the user only partial changes of these when they would
                // expect all updates to be fetched. Instead, nothing is returned.
                (Vec::new(), Vec::new(), Vec::new())
            }
            tl::enums::updates::ChannelDifference::Difference(
                tl::types::updates::ChannelDifference {
                    r#final,
                    pts,
                    timeout: _,
                    new_messages,
                    other_updates: mut updates,
                    chats,
                    users,
                },
            ) => {
                if r#final {
                    debug!(
                        "handling channel {} difference; no longer getting diff",
                        channel_id
                    );
                    self.getting_channel_diff.remove(&channel_id);
                } else {
                    debug!("handling channel {} difference", channel_id);
                }

                self.pts_map.insert(Entry::Channel(channel_id), pts);
                updates.extend(new_messages.into_iter().map(|message| {
                    tl::types::UpdateNewMessage {
                        message,
                        pts: NO_SEQ,
                        pts_count: NO_SEQ,
                    }
                    .into()
                }));

                (updates, users, chats)
            }
        }
    }
}
