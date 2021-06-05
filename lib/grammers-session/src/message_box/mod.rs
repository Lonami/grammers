// Copyright 2020 - developers of the `grammers` project.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// https://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or https://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

//! This module deals with correct handling of updates, including gaps, and knowing when the code
//! should "get difference" (the set of updates that the client should know by now minus the set
//! of updates that it actually knows).
//!
//! Each chat has its own [`Entry`] in the [`MessageBox`] (this `struct` is the "entry point").
//! At any given time, the message box may be either getting difference for them (entry is in
//! [`MessageBox::getting_diff_for`]) or not. If not getting difference, a possible gap may be
//! found for the updates (entry is in [`MessageBox::possible_gaps`]). Otherwise, the entry is
//! on its happy path.
//!
//! Gaps are cleared when they are either resolved on their own (by waiting for a short time)
//! or because we got the difference for the corresponding entry.
//!
//! While there are entries for which their difference must be fetched,
//! [`MessageBox::check_deadlines`] will always return [`Instant::now`], since "now" is the time
//! to get the difference.
mod adaptor;
mod defs;

use super::ChatHashCache;
use crate::message_box::defs::PossibleGap;
use crate::UpdateState;
pub(crate) use defs::Entry;
pub use defs::{Gap, MessageBox};
use defs::{PtsInfo, ResetDeadline, State, NO_SEQ, POSSIBLE_GAP_TIMEOUT};
use grammers_tl_types as tl;
use log::{debug, info, trace, warn};
use std::cmp::Ordering;
use std::collections::{HashMap, HashSet};
use std::mem;
use std::time::{Duration, Instant};

fn next_updates_deadline() -> Instant {
    Instant::now() + defs::NO_UPDATES_TIMEOUT
}

/// Creation, querying, and setting base state.
impl MessageBox {
    /// Create a new, empty [`MessageBox`].
    ///
    /// This is the only way it may return `true` from [`MessageBox::is_empty`].
    pub fn new() -> Self {
        Self {
            map: HashMap::new(),
            date: 1,
            seq: 0,
            next_deadline: None,
            possible_gaps: HashMap::new(),
            getting_diff_for: HashSet::new(),
            reset_deadlines_for: HashSet::new(),
        }
    }

    /// Create a [`MessageBox`] from a previously known update state.
    pub fn load(state: UpdateState) -> Self {
        let deadline = next_updates_deadline();
        let mut map = HashMap::with_capacity(2 + state.channels.len());
        map.insert(
            Entry::AccountWide,
            State {
                pts: state.pts,
                deadline,
            },
        );
        map.insert(
            Entry::SecretChats,
            State {
                pts: state.qts,
                deadline,
            },
        );
        map.extend(
            state
                .channels
                .iter()
                .map(|(&id, &pts)| (Entry::Channel(id), State { pts, deadline })),
        );

        Self {
            map,
            date: state.date,
            seq: state.seq,
            next_deadline: Some(Entry::AccountWide),
            possible_gaps: HashMap::new(),
            getting_diff_for: HashSet::new(),
            reset_deadlines_for: HashSet::new(),
        }
    }

    /// Return the current state in a format that sessions understand.
    ///
    /// This should be used for persisting the state.
    pub fn session_state(&self) -> UpdateState {
        UpdateState {
            pts: self
                .map
                .get(&Entry::AccountWide)
                .map(|s| s.pts)
                .unwrap_or(0),
            qts: self
                .map
                .get(&Entry::SecretChats)
                .map(|s| s.pts)
                .unwrap_or(0),
            date: self.date,
            seq: self.seq,
            channels: self
                .map
                .iter()
                .filter_map(|(entry, s)| match entry {
                    Entry::Channel(id) => Some((*id, s.pts)),
                    _ => None,
                })
                .collect(),
        }
    }

    /// Return true if the message box is empty and has no state yet.
    pub fn is_empty(&self) -> bool {
        self.map
            .get(&Entry::AccountWide)
            .map(|s| s.pts)
            .unwrap_or(NO_SEQ)
            == NO_SEQ
    }

    /// Return the next deadline when receiving updates should timeout.
    ///
    /// If a deadline expired, the corresponding entries will be marked as needing to get its difference.
    /// While there are entries pending of getting their difference, this method returns the current instant.
    pub fn check_deadlines(&mut self) -> Instant {
        let now = Instant::now();

        if !self.getting_diff_for.is_empty() {
            return now;
        }

        let deadline = next_updates_deadline();

        // Most of the time there will be zero or one gap in flight so finding the minimum is cheap.
        let deadline =
            if let Some(gap_deadline) = self.possible_gaps.values().map(|gap| gap.deadline).min() {
                deadline.min(gap_deadline)
            } else if let Some(state) = self.next_deadline.and_then(|entry| self.map.get(&entry)) {
                deadline.min(state.deadline)
            } else {
                deadline
            };

        if now > deadline {
            // Check all expired entries and add them to the list that needs getting difference.
            self.getting_diff_for
                .extend(self.possible_gaps.iter().filter_map(|(entry, gap)| {
                    if now > gap.deadline {
                        info!("gap was not resolved after waiting for {:?}", entry);
                        Some(entry)
                    } else {
                        None
                    }
                }));

            self.getting_diff_for
                .extend(self.map.iter().filter_map(|(entry, state)| {
                    if now > state.deadline {
                        debug!("too much time has passed without updates for {:?}", entry);
                        Some(entry)
                    } else {
                        None
                    }
                }));

            // When extending `getting_diff_for`, it's important to have the moral equivalent of
            // `begin_get_diff` (that is, clear possible gaps if we're now getting difference).
            let possible_gaps = &mut self.possible_gaps;
            self.getting_diff_for.iter().for_each(|entry| {
                possible_gaps.remove(entry);
            });
        }

        deadline
    }

    /// Reset the deadline for the periods without updates for a given entry.
    ///
    /// It also updates the next deadline time to reflect the new closest deadline.
    fn reset_deadline(&mut self, entry: Entry, deadline: Instant) {
        if let Some(state) = self.map.get_mut(&entry) {
            state.deadline = deadline;
            debug!("reset deadline {:?} for {:?}", deadline, entry);
        } else {
            // TODO figure out why this happens
            info!("did not reset deadline for {:?} as it had no entry", entry);
        }

        if self.next_deadline == Some(entry) {
            // If the updated deadline was the closest one, recalculate the new minimum.
            self.next_deadline = Some(
                *self
                    .map
                    .iter()
                    .min_by_key(|(_, state)| state.deadline)
                    .unwrap()
                    .0,
            );
        } else if self
            .next_deadline
            .map(|e| deadline < self.map[&e].deadline)
            .unwrap_or(false)
        {
            // If the updated deadline is smaller than the next deadline, change the next deadline to be the new one.
            // An unrelated deadline was updated, so the closest one remains unchanged.
            self.next_deadline = Some(entry);
        }
    }

    /// Convenience to reset a channel's deadline, with optional timeout.
    fn reset_channel_deadline(&mut self, channel_id: i32, timeout: Option<i32>) {
        self.reset_deadline(
            Entry::Channel(channel_id),
            Instant::now()
                + timeout
                    .map(|t| Duration::from_secs(t as _))
                    .unwrap_or(defs::NO_UPDATES_TIMEOUT),
        );
    }

    /// Reset all the deadlines in `reset_deadlines_for` and then empty the set.
    fn apply_deadlines_reset(&mut self) {
        let next_deadline = next_updates_deadline();
        let mut reset_deadlines_for = mem::take(&mut self.reset_deadlines_for);
        reset_deadlines_for
            .iter()
            .for_each(|&entry| self.reset_deadline(entry, next_deadline));
        reset_deadlines_for.clear();
        self.reset_deadlines_for = reset_deadlines_for;
    }

    /// Sets the update state.
    ///
    /// Should be called right after login if [`MessageBox::new`] was used, otherwise undesirable
    /// updates will be fetched.
    pub fn set_state(&mut self, state: tl::enums::updates::State) {
        let deadline = next_updates_deadline();
        let state: tl::types::updates::State = state.into();
        self.map.insert(
            Entry::AccountWide,
            State {
                pts: state.pts,
                deadline,
            },
        );
        self.map.insert(
            Entry::SecretChats,
            State {
                pts: state.qts,
                deadline,
            },
        );
        self.date = state.date;
        self.seq = state.seq;
    }

    /// Like [`MessageBox::set_state`], but for channels. Useful when getting dialogs.
    ///
    /// The update state will only be updated if no entry was known previously.
    pub fn try_set_channel_state(&mut self, id: i32, pts: i32) {
        self.map.entry(Entry::Channel(id)).or_insert_with(|| State {
            pts,
            deadline: next_updates_deadline(),
        });
    }

    /// Begin getting difference for the given entry.
    ///
    /// Clears any previous gaps.
    fn begin_get_diff(&mut self, entry: Entry) {
        self.getting_diff_for.insert(entry);
        self.possible_gaps.remove(&entry);
    }

    /// Finish getting difference for the given entry.
    ///
    /// It also resets the deadline.
    fn end_get_diff(&mut self, entry: Entry) {
        self.getting_diff_for.remove(&entry);
        self.reset_deadline(entry, next_updates_deadline());
        assert!(
            !self.possible_gaps.contains_key(&entry),
            "gaps shouldn't be created while getting difference"
        );
    }
}

// "Normal" updates flow (processing and detection of gaps).
impl MessageBox {
    /// Process an update and return what should be done with it.
    ///
    /// Updates corresponding to entries for which their difference is currently being fetched
    /// will be ignored. While according to the [updates' documentation]:
    ///
    /// > Implementations [have] to postpone updates received via the socket while
    /// > filling gaps in the event and `Update` sequences, as well as avoid filling
    /// > gaps in the same sequence.
    ///
    /// In practice, these updates should have also been retrieved through getting difference.
    ///
    /// [updates documentation] https://core.telegram.org/api/updates
    pub fn process_updates(
        &mut self,
        updates: tl::enums::Updates,
        chat_hashes: &mut ChatHashCache,
        result: &mut Vec<tl::enums::Update>,
    ) -> Result<(Vec<tl::enums::User>, Vec<tl::enums::Chat>), Gap> {
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
                self.begin_get_diff(Entry::AccountWide);
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
                    return Ok((users, chats));
                }
                Ordering::Less => {
                    debug!(
                        "gap detected (local seq {}, remote seq {})",
                        self.seq, seq_start
                    );
                    self.begin_get_diff(Entry::AccountWide);
                    return Err(Gap);
                }
            }

            self.date = date;
            if seq != NO_SEQ {
                self.seq = seq;
                trace!("updated date = {}, seq = {}", date, seq);
            }
        }

        result.extend(
            updates
                .into_iter()
                .filter_map(|u| self.apply_pts_info(u, ResetDeadline::Yes)),
        );

        self.apply_deadlines_reset();

        if !self.possible_gaps.is_empty() {
            // For each update in possible gaps, see if the gap has been resolved already.
            let keys = self.possible_gaps.keys().copied().collect::<Vec<_>>();
            for key in keys {
                self.possible_gaps
                    .get_mut(&key)
                    .unwrap()
                    .updates
                    .sort_by_key(|update| match PtsInfo::from_update(update) {
                        Some(pts) => (pts.pts - pts.pts_count),
                        None => 0,
                    });

                for _ in 0..self.possible_gaps[&key].updates.len() {
                    let update = self.possible_gaps.get_mut(&key).unwrap().updates.remove(0);
                    // If this fails to apply, it will get re-inserted at the end.
                    // All should fail, so the order will be preserved (it would've cycled once).
                    if let Some(update) = self.apply_pts_info(update, ResetDeadline::No) {
                        result.push(update);
                    }
                }
            }

            // Clear now-empty gaps.
            self.possible_gaps.retain(|_, v| !v.updates.is_empty());
            if self.possible_gaps.is_empty() {
                debug!("successfully resolved gap by waiting");
            }
        }

        Ok((users, chats))
    }

    /// Tries to apply the input update if its `PtsInfo` follows the correct order.
    ///
    /// If the update can be applied, it is returned; otherwise, the update is stored in a
    /// possible gap (unless it was already handled or would be handled through getting
    /// difference) and `None` is returned.
    fn apply_pts_info(
        &mut self,
        update: tl::enums::Update,
        reset_deadline: ResetDeadline,
    ) -> Option<tl::enums::Update> {
        let pts = match PtsInfo::from_update(&update) {
            Some(pts) => pts,
            // No pts means that the update can be applied in any order.
            None => return Some(update),
        };

        // As soon as we receive an update of any form related to messages (has `PtsInfo`),
        // the "no updates" period for that entry is reset.
        //
        // Build the `HashSet` to avoid calling `reset_deadline` more than once for the same entry.
        if reset_deadline == ResetDeadline::Yes {
            self.reset_deadlines_for.insert(pts.entry);
        }

        if self.getting_diff_for.contains(&pts.entry) {
            debug!(
                "skipping update for {:?} (getting difference, count {:?}, remote {:?})",
                pts.entry, pts.pts_count, pts.pts
            );
            // Note: early returning here also prevents gap from being inserted (which they should
            // not be while getting difference).
            return None;
        }

        let local_pts = if let Some(state) = self.map.get(&pts.entry) {
            let local_pts = state.pts;
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
                    self.possible_gaps
                        .entry(pts.entry)
                        .or_insert_with(|| PossibleGap {
                            deadline: Instant::now() + POSSIBLE_GAP_TIMEOUT,
                            updates: Vec::new(),
                        })
                        .updates
                        .push(update);

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
        self.map
            .entry(pts.entry)
            .or_insert_with(|| State {
                pts: local_pts + pts.pts_count,
                deadline: next_updates_deadline(),
            })
            .pts = local_pts + pts.pts_count;

        Some(update)
    }
}

/// Getting and applying account difference.
impl MessageBox {
    /// Return the request that needs to be made to get the difference, if any.
    pub fn get_difference(&mut self) -> Option<tl::functions::updates::GetDifference> {
        let entry = Entry::AccountWide;
        if self.getting_diff_for.contains(&entry) {
            if let Some(state) = self.map.get(&entry) {
                return Some(tl::functions::updates::GetDifference {
                    pts: state.pts,
                    pts_total_limit: None,
                    date: self.date,
                    qts: self.map[&Entry::SecretChats].pts,
                });
            } else {
                // TODO investigate when/why/if this can happen
                warn!("cannot getDifference as we're missing account pts");
                self.end_get_diff(entry);
            }
        }
        None
    }

    /// Similar to [`MessageBox::process_updates`], but using the result from getting difference.
    pub fn apply_difference(
        &mut self,
        difference: tl::enums::updates::Difference,
        chat_hashes: &mut ChatHashCache,
    ) -> (
        Vec<tl::enums::Update>,
        Vec<tl::enums::User>,
        Vec<tl::enums::Chat>,
    ) {
        match difference {
            tl::enums::updates::Difference::Empty(diff) => {
                debug!(
                    "handling empty difference (date = {}, seq = {}); no longer getting diff",
                    diff.date, diff.seq
                );
                self.date = diff.date;
                self.seq = diff.seq;
                self.end_get_diff(Entry::AccountWide);
                (Vec::new(), Vec::new(), Vec::new())
            }
            tl::enums::updates::Difference::Difference(diff) => {
                debug!(
                    "handling full difference {:?}; no longer getting diff",
                    diff.state
                );
                self.end_get_diff(Entry::AccountWide);
                chat_hashes.extend(&diff.users, &diff.chats);
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
                chat_hashes.extend(&users, &chats);
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
                // TODO when are deadlines reset if we update the map??
                self.map.get_mut(&Entry::AccountWide).unwrap().pts = diff.pts;
                self.end_get_diff(Entry::AccountWide);
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
        self.map.get_mut(&Entry::AccountWide).unwrap().pts = state.pts;
        self.map.get_mut(&Entry::SecretChats).unwrap().pts = state.qts;
        self.date = state.date;
        self.seq = state.seq;

        updates.iter().for_each(|u| match u {
            tl::enums::Update::ChannelTooLong(c) => {
                // `c.pts`, if any, is the channel's current `pts`; we do not need this.
                info!("got {:?} during getDifference", c);
                self.begin_get_diff(Entry::Channel(c.channel_id));
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
    pub fn get_channel_difference(
        &mut self,
        chat_hashes: &ChatHashCache,
    ) -> Option<tl::functions::updates::GetChannelDifference> {
        let (entry, id) = self
            .getting_diff_for
            .iter()
            .find_map(|&entry| match entry {
                Entry::Channel(id) => Some((entry, id)),
                _ => None,
            })?;

        if let Some(channel) = chat_hashes.get_input_channel(id) {
            if let Some(state) = self.map.get(&entry) {
                Some(tl::functions::updates::GetChannelDifference {
                    force: false,
                    channel,
                    filter: tl::enums::ChannelMessagesFilter::Empty,
                    pts: state.pts,
                    limit: if chat_hashes.is_self_bot() {
                        defs::BOT_CHANNEL_DIFF_LIMIT
                    } else {
                        defs::USER_CHANNEL_DIFF_LIMIT
                    },
                })
            } else {
                // TODO investigate when/why/if this can happen
                warn!(
                    "cannot getChannelDifference for {} as we're missing its pts",
                    id
                );
                self.end_get_diff(entry);
                None
            }
        } else {
            warn!(
                "cannot getChannelDifference for {} as we're missing its hash",
                id
            );
            self.end_get_diff(entry);
            // Remove the outdated `pts` entry from the map so that the next update can correct
            // it. Otherwise, it will spam that the access hash is missing.
            self.map.remove(&entry);
            None
        }
    }

    /// Similar to [`MessageBox::process_updates`], but using the result from getting difference.
    pub fn apply_channel_difference(
        &mut self,
        request: tl::functions::updates::GetChannelDifference,
        difference: tl::enums::updates::ChannelDifference,
        chat_hashes: &mut ChatHashCache,
    ) -> (
        Vec<tl::enums::Update>,
        Vec<tl::enums::User>,
        Vec<tl::enums::Chat>,
    ) {
        let channel_id = match request.channel {
            tl::enums::InputChannel::Channel(c) => c.channel_id,
            _ => panic!("request had wrong input channel"),
        };
        let entry = Entry::Channel(channel_id);

        self.possible_gaps.remove(&entry);

        match difference {
            tl::enums::updates::ChannelDifference::Empty(diff) => {
                assert!(diff.r#final);
                debug!(
                    "handling empty channel {} difference (pts = {}); no longer getting diff",
                    channel_id, diff.pts
                );
                self.end_get_diff(entry);
                self.map.get_mut(&entry).unwrap().pts = diff.pts;
                (Vec::new(), Vec::new(), Vec::new())
            }
            tl::enums::updates::ChannelDifference::TooLong(diff) => {
                assert!(diff.r#final);
                info!(
                    "handling too long channel {} difference; no longer getting diff",
                    channel_id
                );
                match diff.dialog {
                    tl::enums::Dialog::Dialog(d) => {
                        self.map.get_mut(&entry).unwrap().pts = d.pts.expect(
                            "channelDifferenceTooLong dialog did not actually contain a pts",
                        );
                    }
                    tl::enums::Dialog::Folder(_) => {
                        panic!("received a folder on channelDifferenceTooLong")
                    }
                }
                chat_hashes.extend(&diff.users, &diff.chats);
                self.reset_channel_deadline(channel_id, diff.timeout);
                // This `diff` has the "latest messages and corresponding chats", but it would
                // be strange to give the user only partial changes of these when they would
                // expect all updates to be fetched. Instead, nothing is returned.
                (Vec::new(), Vec::new(), Vec::new())
            }
            tl::enums::updates::ChannelDifference::Difference(
                tl::types::updates::ChannelDifference {
                    r#final,
                    pts,
                    timeout,
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
                    self.end_get_diff(entry);
                } else {
                    debug!("handling channel {} difference", channel_id);
                }

                self.map.get_mut(&entry).unwrap().pts = pts;
                updates.extend(new_messages.into_iter().map(|message| {
                    tl::types::UpdateNewMessage {
                        message,
                        pts: NO_SEQ,
                        pts_count: NO_SEQ,
                    }
                    .into()
                }));
                chat_hashes.extend(&users, &chats);
                self.reset_channel_deadline(channel_id, timeout);

                (updates, users, chats)
            }
        }
    }
}
