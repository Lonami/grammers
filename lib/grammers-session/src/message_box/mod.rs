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
use crate::generated::enums::ChannelState as ChannelStateEnum;
use crate::generated::types::ChannelState;
use crate::message_box::defs::PossibleGap;
use crate::UpdateState;
pub(crate) use defs::Entry;
pub use defs::{Gap, MessageBox};
use defs::{PtsInfo, State, NO_DATE, NO_PTS, NO_SEQ, POSSIBLE_GAP_TIMEOUT};
use grammers_tl_types as tl;
use log::{debug, info, trace, warn};
use std::cmp::Ordering;
use std::collections::{HashMap, HashSet};
use std::mem;
use std::time::{Duration, Instant};
use tl::enums::InputChannel;

fn next_updates_deadline() -> Instant {
    Instant::now() + defs::NO_UPDATES_TIMEOUT
}

#[allow(clippy::new_without_default)]
/// Creation, querying, and setting base state.
impl MessageBox {
    /// Create a new, empty [`MessageBox`].
    ///
    /// This is the only way it may return `true` from [`MessageBox::is_empty`].
    pub fn new() -> Self {
        trace!("created new message box with no previous state");
        Self {
            map: HashMap::new(),
            date: 1, // non-zero or getting difference will fail
            seq: NO_SEQ,
            possible_gaps: HashMap::new(),
            getting_diff_for: HashSet::new(),
            next_deadline: None,
            tmp_entries: HashSet::new(),
        }
    }

    /// Create a [`MessageBox`] from a previously known update state.
    pub fn load(state: UpdateState) -> Self {
        trace!("created new message box with state: {:?}", state);
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
        map.extend(state.channels.iter().map(|ChannelStateEnum::State(c)| {
            (
                Entry::Channel(c.channel_id),
                State {
                    pts: c.pts,
                    deadline,
                },
            )
        }));

        Self {
            map,
            date: state.date,
            seq: state.seq,
            possible_gaps: HashMap::new(),
            getting_diff_for: HashSet::new(),
            next_deadline: Some(Entry::AccountWide),
            tmp_entries: HashSet::new(),
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
                .unwrap_or(NO_PTS),
            qts: self
                .map
                .get(&Entry::SecretChats)
                .map(|s| s.pts)
                .unwrap_or(NO_PTS),
            date: self.date,
            seq: self.seq,
            channels: self
                .map
                .iter()
                .filter_map(|(entry, s)| match entry {
                    Entry::Channel(id) => Some(
                        ChannelState {
                            channel_id: *id,
                            pts: s.pts,
                        }
                        .into(),
                    ),
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
            .unwrap_or(NO_PTS)
            == NO_PTS
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

        if now >= deadline {
            // Check all expired entries and add them to the list that needs getting difference.
            self.getting_diff_for
                .extend(self.possible_gaps.iter().filter_map(|(entry, gap)| {
                    if now >= gap.deadline {
                        info!("gap was not resolved after waiting for {:?}", entry);
                        Some(entry)
                    } else {
                        None
                    }
                }));

            self.getting_diff_for
                .extend(self.map.iter().filter_map(|(entry, state)| {
                    if now >= state.deadline {
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

    /// Reset the deadline for the periods without updates for all input entries.
    ///
    /// It also updates the next deadline time to reflect the new closest deadline.
    fn reset_deadlines(&mut self, entries: &HashSet<Entry>, deadline: Instant) {
        if entries.is_empty() {
            return;
        }
        for entry in entries {
            if let Some(state) = self.map.get_mut(entry) {
                state.deadline = deadline;
                debug!("reset deadline {:?} for {:?}", deadline, entry);
            } else {
                panic!("did not reset deadline for {:?} as it had no entry", entry);
            }
        }

        if self
            .next_deadline
            .as_ref()
            .map(|next| entries.contains(next))
            .unwrap_or(false)
        {
            // If the updated deadline was the closest one, recalculate the new minimum.
            self.next_deadline = Some(
                self.map
                    .iter()
                    .min_by_key(|(_, state)| state.deadline)
                    .map(|i| *i.0)
                    .expect("deadline should exist"),
            );
        } else if self
            .next_deadline
            .map(|e| deadline < self.map[&e].deadline)
            .unwrap_or(false)
        {
            // If the updated deadline is smaller than the next deadline, change the next deadline to be the new one.
            // An unrelated deadline was updated, so the closest one remains unchanged.
            // Any entry will do, as they all share the same new deadline.
            self.next_deadline = Some(*entries.iter().next().unwrap());
        }
    }

    /// Convenience to reset a single entry's deadline.
    fn reset_deadline(&mut self, entry: Entry, deadline: Instant) {
        let mut entries = mem::take(&mut self.tmp_entries);
        entries.insert(entry);
        self.reset_deadlines(&entries, deadline);
        entries.clear();
        self.tmp_entries = entries;
    }

    /// Convenience to reset a channel's deadline, with optional timeout.
    fn reset_channel_deadline(&mut self, channel_id: i64, timeout: Option<i32>) {
        self.reset_deadline(
            Entry::Channel(channel_id),
            Instant::now()
                + timeout
                    .map(|t| Duration::from_secs(t as _))
                    .unwrap_or(defs::NO_UPDATES_TIMEOUT),
        );
    }

    /// Sets the update state.
    ///
    /// Should be called right after login if [`MessageBox::new`] was used, otherwise undesirable
    /// updates will be fetched.
    pub fn set_state(&mut self, state: tl::enums::updates::State) {
        trace!("setting state {:?}", state);
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
    pub fn try_set_channel_state(&mut self, id: i64, pts: i32) {
        trace!("trying to set channel state for {}: {}", id, pts);
        self.map.entry(Entry::Channel(id)).or_insert_with(|| State {
            pts,
            deadline: next_updates_deadline(),
        });
    }

    /// Try to begin getting difference for the given entry.
    /// Fails if the entry does not have a previously-known state that can be used to get its difference.
    ///
    /// Clears any previous gaps.
    fn try_begin_get_diff(&mut self, entry: Entry) {
        if !self.map.contains_key(&entry) {
            // Won't actually be able to get difference for this entry if we don't have a pts to start off from.
            if self.possible_gaps.contains_key(&entry) {
                panic!(
                    "Should not have a possible_gap for an entry {:?} not in the state map",
                    entry
                );
            }
            return;
        }

        self.getting_diff_for.insert(entry);
        self.possible_gaps.remove(&entry);
    }

    /// Finish getting difference for the given entry.
    ///
    /// It also resets the deadline.
    fn end_get_diff(&mut self, entry: Entry) {
        if !self.getting_diff_for.remove(&entry) {
            panic!("Called end_get_diff on an entry which was not getting diff for");
        };
        self.reset_deadline(entry, next_updates_deadline());
        assert!(
            !self.possible_gaps.contains_key(&entry),
            "gaps shouldn't be created while getting difference"
        );
    }
}

// "Normal" updates flow (processing and detection of gaps).
impl MessageBox {
    /// Make sure all peer hashes contained in the update are known by the client
    /// (either by checking if they were already known, or by extending the hash cache
    /// with those that were not known).
    ///
    /// If a peer is found, but it doesn't contain a non-`min` hash and no hash for it
    /// is known, it is treated as a gap.
    pub fn ensure_known_peer_hashes(
        &mut self,
        updates: &tl::enums::Updates,
        chat_hashes: &mut ChatHashCache,
    ) -> Result<(), Gap> {
        // In essence, "min constructors suck".
        // Apparently, TDLib just does `getDifference` if encounters non-cached min peers.
        // So rather than using the `inputPeer*FromMessage` (which not only are considerably
        // larger but may need to be nested, and may stop working if the message is gone),
        // just treat it as a gap when encountering peers for which the hash is not known.
        // Context: https://t.me/tdlibchat/15096.
        if chat_hashes.extend_from_updates(updates) {
            Ok(())
        } else {
            // However, some updates do not change the pts, so attempting to recover the gap
            // will just result in an empty result from `getDifference` (being just wasteful).
            // Check if this update has any pts we can try to recover from.
            let can_recover = match updates {
                tl::enums::Updates::TooLong => true,
                tl::enums::Updates::UpdateShortMessage(_) => true,
                tl::enums::Updates::UpdateShortChatMessage(_) => true,
                tl::enums::Updates::UpdateShort(u) => PtsInfo::from_update(&u.update).is_some(),
                tl::enums::Updates::Combined(_) => true,
                tl::enums::Updates::Updates(_) => true,
                tl::enums::Updates::UpdateShortSentMessage(_) => true,
            };

            if can_recover {
                info!("received an update referencing an unknown peer, treating as gap");
                self.try_begin_get_diff(Entry::AccountWide);
                Err(Gap)
            } else {
                info!("received an update referencing an unknown peer, but cannot find out who");
                Ok(())
            }
        }
    }

    /// Process an update and return what should be done with it.
    ///
    /// Updates corresponding to entries for which their difference is currently being fetched
    /// will be ignored. While according to the [updates' documentation]:
    ///
    /// > Implementations \[have\] to postpone updates received via the socket while
    /// > filling gaps in the event and `Update` sequences, as well as avoid filling
    /// > gaps in the same sequence.
    ///
    /// In practice, these updates should have also been retrieved through getting difference.
    ///
    /// [updates' documentation]: https://core.telegram.org/api/updates
    pub fn process_updates(
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
        trace!("processing updates: {:?}", updates);
        // Top level, when handling received `updates` and `updatesCombined`.
        // `updatesCombined` groups all the fields we care about, which is why we use it.
        //
        // This assumes all access hashes are already known to the client (socket updates are
        // expected to use `ensure_known_peer_hashes`, and the result from getting difference
        // has to deal with the peers in a different way).
        let tl::types::UpdatesCombined {
            date,
            seq_start,
            seq,
            mut updates,
            users,
            chats,
        } = match adaptor::adapt(updates, chat_hashes) {
            Ok(combined) => combined,
            Err(Gap) => {
                self.try_begin_get_diff(Entry::AccountWide);
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
                    self.try_begin_get_diff(Entry::AccountWide);
                    return Err(Gap);
                }
            }
        }

        fn update_sort_key(update: &tl::enums::Update) -> i32 {
            match PtsInfo::from_update(update) {
                Some(pts) => pts.pts - pts.pts_count,
                None => NO_PTS,
            }
        }

        // Telegram can send updates out of order (e.g. `ReadChannelInbox` first
        // and then `NewChannelMessage`, both with the same `pts`, but the `count`
        // is `0` and `1` respectively), so we sort them first.
        updates.sort_by_key(update_sort_key);

        // Adding `possible_gaps.len()` is a guesstimate. Often it's just one update.
        let mut result = Vec::with_capacity(updates.len() + self.possible_gaps.len());

        // This loop does a lot at once to reduce the amount of times we need to iterate over
        // the updates as an optimization.
        //
        // It mutates the local pts state, remembers possible gaps, builds a set of entries for
        // which the deadlines should be reset, and determines whether any local pts was changed
        // so that the seq can be updated too (which could otherwise have been done earlier).
        let mut any_pts_applied = false;
        let mut reset_deadlines_for = mem::take(&mut self.tmp_entries);
        for update in updates {
            let (entry, update) = self.apply_pts_info(update);
            if let Some(entry) = entry {
                // As soon as we receive an update of any form related to messages (has `PtsInfo`),
                // the "no updates" period for that entry is reset. All the deadlines are reset at
                // once via the temporary entries buffer as an optimization.
                reset_deadlines_for.insert(entry);
            }
            if let Some(update) = update {
                result.push(update);
                any_pts_applied |= entry.is_some();
            }
        }
        self.reset_deadlines(&reset_deadlines_for, next_updates_deadline());
        reset_deadlines_for.clear();
        self.tmp_entries = reset_deadlines_for;

        // > If the updates were applied, local *Updates* state must be updated
        // > with `seq` (unless it's 0) and `date` from the constructor.
        //
        // By "were applied", we assume it means "some other pts was applied".
        // Updates which can be applied in any order, such as `UpdateChat`,
        // should not cause `seq` to be updated (or upcoming updates such as
        // `UpdateChatParticipant` could be missed).
        if any_pts_applied {
            if date != NO_DATE {
                self.date = date;
            }
            if seq != NO_SEQ {
                self.seq = seq;
            }
        }

        if !self.possible_gaps.is_empty() {
            // For each update in possible gaps, see if the gap has been resolved already.
            let keys = self.possible_gaps.keys().copied().collect::<Vec<_>>();
            for key in keys {
                self.possible_gaps
                    .get_mut(&key)
                    .unwrap()
                    .updates
                    .sort_by_key(update_sort_key);

                for _ in 0..self.possible_gaps[&key].updates.len() {
                    let update = self.possible_gaps.get_mut(&key).unwrap().updates.remove(0);
                    // If this fails to apply, it will get re-inserted at the end.
                    // All should fail, so the order will be preserved (it would've cycled once).
                    if let (_, Some(update)) = self.apply_pts_info(update) {
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

        Ok((result, users, chats))
    }

    /// Tries to apply the input update if its `PtsInfo` follows the correct order.
    ///
    /// If the update can be applied, it is returned; otherwise, the update is stored in a
    /// possible gap (unless it was already handled or would be handled through getting
    /// difference) and `None` is returned.
    fn apply_pts_info(
        &mut self,
        update: tl::enums::Update,
    ) -> (Option<Entry>, Option<tl::enums::Update>) {
        if let tl::enums::Update::ChannelTooLong(u) = update {
            self.try_begin_get_diff(Entry::Channel(u.channel_id));
            return (None, None);
        }

        let pts = match PtsInfo::from_update(&update) {
            Some(pts) => pts,
            // No pts means that the update can be applied in any order.
            None => return (None, Some(update)),
        };

        if self.getting_diff_for.contains(&pts.entry) {
            debug!(
                "skipping update for {:?} (getting difference, count {:?}, remote {:?})",
                pts.entry, pts.pts_count, pts.pts
            );
            // Note: early returning here also prevents gap from being inserted (which they should
            // not be while getting difference).
            return (Some(pts.entry), None);
        }

        if let Some(state) = self.map.get(&pts.entry) {
            let local_pts = state.pts;
            match (local_pts + pts.pts_count).cmp(&pts.pts) {
                // Apply
                Ordering::Equal => {}
                // Ignore
                Ordering::Greater => {
                    debug!(
                        "skipping update for {:?} (local {:?}, count {:?}, remote {:?})",
                        pts.entry, local_pts, pts.pts_count, pts.pts
                    );
                    return (Some(pts.entry), None);
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

                    return (Some(pts.entry), None);
                }
            }
        }
        // else, there is no previous `pts` known, and because this update has to be "right"
        // (it's the first one) our `local_pts` must be `pts - pts_count`.

        self.map
            .entry(pts.entry)
            .or_insert_with(|| State {
                pts: NO_PTS,
                deadline: next_updates_deadline(),
            })
            .pts = pts.pts;

        (Some(pts.entry), Some(update))
    }
}

/// Getting and applying account difference.
impl MessageBox {
    /// Return the request that needs to be made to get the difference, if any.
    pub fn get_difference(&mut self) -> Option<tl::functions::updates::GetDifference> {
        for entry in [Entry::AccountWide, Entry::SecretChats] {
            if self.getting_diff_for.contains(&entry) {
                if !self.map.contains_key(&entry) {
                    panic!(
                        "Should not try to get difference for an entry {:?} without known state",
                        entry
                    );
                }

                let gd = tl::functions::updates::GetDifference {
                    pts: self.map[&Entry::AccountWide].pts,
                    pts_limit: None,
                    pts_total_limit: None,
                    date: self.date,
                    qts: if self.map.contains_key(&Entry::SecretChats) {
                        self.map[&Entry::SecretChats].pts
                    } else {
                        NO_PTS
                    },
                    qts_limit: None,
                };
                trace!("requesting {:?}", gd);
                return Some(gd);
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
        trace!("applying account difference: {:?}", difference);
        let finish: bool;
        let result = match difference {
            tl::enums::updates::Difference::Empty(diff) => {
                debug!(
                    "handling empty difference (date = {}, seq = {}); no longer getting diff",
                    diff.date, diff.seq
                );
                finish = true;
                self.date = diff.date;
                self.seq = diff.seq;
                (Vec::new(), Vec::new(), Vec::new())
            }
            tl::enums::updates::Difference::Difference(diff) => {
                // TODO return Err(attempt to find users)
                let _ = chat_hashes.extend(&diff.users, &diff.chats);

                debug!(
                    "handling full difference {:?}; no longer getting diff",
                    diff.state
                );
                finish = true;
                self.apply_difference_type(diff, chat_hashes)
            }
            tl::enums::updates::Difference::Slice(tl::types::updates::DifferenceSlice {
                new_messages,
                new_encrypted_messages,
                other_updates,
                chats,
                users,
                intermediate_state: state,
            }) => {
                // TODO return Err(attempt to find users)
                let _ = chat_hashes.extend(&users, &chats);

                debug!("handling partial difference {:?}", state);
                finish = false;
                self.apply_difference_type(
                    tl::types::updates::Difference {
                        new_messages,
                        new_encrypted_messages,
                        other_updates,
                        chats,
                        users,
                        state,
                    },
                    chat_hashes,
                )
            }
            tl::enums::updates::Difference::TooLong(diff) => {
                debug!(
                    "handling too-long difference (pts = {}); no longer getting diff",
                    diff.pts
                );
                finish = true;
                // the deadline will be reset once the diff ends
                self.map.get_mut(&Entry::AccountWide).unwrap().pts = diff.pts;
                (Vec::new(), Vec::new(), Vec::new())
            }
        };

        if finish {
            let account = self.getting_diff_for.contains(&Entry::AccountWide);
            let secret = self.getting_diff_for.contains(&Entry::SecretChats);

            if !account && !secret {
                panic!("Should not be applying the difference when neither account or secret diff was active")
            }

            if account {
                self.end_get_diff(Entry::AccountWide);
            }
            if secret {
                self.end_get_diff(Entry::SecretChats);
            }
        }

        result
    }

    fn apply_difference_type(
        &mut self,
        tl::types::updates::Difference {
            new_messages,
            new_encrypted_messages,
            other_updates: updates,
            chats,
            users,
            state: tl::enums::updates::State::State(state),
        }: tl::types::updates::Difference,
        chat_hashes: &mut ChatHashCache,
    ) -> (
        Vec<tl::enums::Update>,
        Vec<tl::enums::User>,
        Vec<tl::enums::Chat>,
    ) {
        self.map.get_mut(&Entry::AccountWide).unwrap().pts = state.pts;
        self.map.get_mut(&Entry::SecretChats).unwrap().pts = state.qts;
        self.date = state.date;
        self.seq = state.seq;

        // other_updates can contain things like UpdateChannelTooLong and UpdateNewChannelMessage.
        // We need to process those as if they were socket updates to discard any we have already handled.
        let us = tl::enums::Updates::Updates(tl::types::Updates {
            updates,
            users,
            chats,
            date: NO_DATE,
            seq: NO_SEQ,
        });

        // It is possible that the result from `GetDifference` includes users with `min = true`.
        // TODO in that case, we will have to resort to getUsers.
        let (mut result_updates, users, chats) = self
            .process_updates(us, chat_hashes)
            .expect("gap is detected while applying difference");

        result_updates.extend(
            new_messages
                .into_iter()
                .map(|message| {
                    tl::types::UpdateNewMessage {
                        message,
                        pts: NO_PTS,
                        pts_count: 0,
                    }
                    .into()
                })
                .chain(new_encrypted_messages.into_iter().map(|message| {
                    tl::types::UpdateNewEncryptedMessage {
                        message,
                        qts: NO_PTS,
                    }
                    .into()
                })),
        );

        (result_updates, users, chats)
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

        if let Some(packed) = chat_hashes.get(id) {
            let channel = tl::types::InputChannel {
                channel_id: packed.id,
                access_hash: packed
                    .access_hash
                    .expect("chat_hashes had chat without hash"),
            }
            .into();
            if let Some(state) = self.map.get(&entry) {
                let gd = tl::functions::updates::GetChannelDifference {
                    force: false,
                    channel,
                    filter: tl::enums::ChannelMessagesFilter::Empty,
                    pts: state.pts,
                    limit: if chat_hashes.is_self_bot() {
                        defs::BOT_CHANNEL_DIFF_LIMIT
                    } else {
                        defs::USER_CHANNEL_DIFF_LIMIT
                    },
                };
                trace!("requesting {:?}", gd);
                Some(gd)
            } else {
                panic!(
                    "Should not try to get difference for an entry {:?} without known state",
                    entry
                );
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
        let channel_id = channel_id(&request).expect("request had wrong input channel");
        trace!(
            "applying channel difference for {}: {:?}",
            channel_id,
            difference
        );
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
                // TODO return Err(attempt to find users)
                let _ = chat_hashes.extend(&diff.users, &diff.chats);

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
                    other_updates: updates,
                    chats,
                    users,
                },
            ) => {
                // TODO return Err(attempt to find users)
                let _ = chat_hashes.extend(&users, &chats);

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
                let us = tl::enums::Updates::Updates(tl::types::Updates {
                    updates,
                    users,
                    chats,
                    date: NO_DATE,
                    seq: NO_SEQ,
                });
                let (mut result_updates, users, chats) = self
                    .process_updates(us, chat_hashes)
                    .expect("gap is detected while applying channel difference");

                result_updates.extend(new_messages.into_iter().map(|message| {
                    tl::types::UpdateNewChannelMessage {
                        message,
                        pts: NO_PTS,
                        pts_count: 0,
                    }
                    .into()
                }));
                self.reset_channel_deadline(channel_id, timeout);

                (result_updates, users, chats)
            }
        }
    }

    pub fn end_channel_difference(
        &mut self,
        request: &tl::functions::updates::GetChannelDifference,
        reason: PrematureEndReason,
    ) {
        if let Some(channel_id) = channel_id(request) {
            trace!(
                "ending channel difference for {} because {:?}",
                channel_id,
                reason
            );
            let entry = Entry::Channel(channel_id);
            match reason {
                PrematureEndReason::TemporaryServerIssues => {
                    self.possible_gaps.remove(&entry);
                    self.end_get_diff(entry);
                }
                PrematureEndReason::Banned => {
                    self.possible_gaps.remove(&entry);
                    self.end_get_diff(entry);
                    self.map.remove(&entry);
                }
            }
        };
    }
}

pub fn channel_id(request: &tl::functions::updates::GetChannelDifference) -> Option<i64> {
    match request.channel {
        InputChannel::Channel(ref c) => Some(c.channel_id),
        InputChannel::FromMessage(ref c) => Some(c.channel_id),
        InputChannel::Empty => None,
    }
}

#[derive(Debug)]
pub enum PrematureEndReason {
    TemporaryServerIssues,
    Banned,
}
