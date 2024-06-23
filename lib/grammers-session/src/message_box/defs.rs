// Copyright 2020 - developers of the `grammers` project.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// https://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or https://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.
use grammers_tl_types as tl;
use std::collections::{HashMap, HashSet};
use std::time::{Duration, Instant};

/// Telegram sends `seq` equal to `0` when "it doesn't matter", so we use that value too.
pub(super) const NO_SEQ: i32 = 0;

/// It has been observed that Telegram may send updates with `qts` equal to `0` (for
/// example with `ChannelParticipant`), interleaved with non-zero `qts` values. This
/// presumably means that the ordering should be "ignored" in that case.
///
/// One can speculate this is done because the field is not optional in the TL definition.
///
/// Not ignoring the `pts` information in those updates can lead to failures resolving gaps.
pub(super) const NO_PTS: i32 = 0;

/// Non-update types like `messages.affectedMessages` can contain `pts` that should still be
/// processed. Because there's no `date`, a value of `0` is used as the sentinel value for
/// the `date` when constructing the dummy `Updates` (in order to handle them uniformly).
pub(super) const NO_DATE: i32 = 0;

// See https://core.telegram.org/method/updates.getChannelDifference.
pub(super) const BOT_CHANNEL_DIFF_LIMIT: i32 = 100000;
pub(super) const USER_CHANNEL_DIFF_LIMIT: i32 = 100;

// > It may be useful to wait up to 0.5 seconds
pub(super) const POSSIBLE_GAP_TIMEOUT: Duration = Duration::from_millis(500);

/// After how long without updates the client will "timeout".
///
/// When this timeout occurs, the client will attempt to fetch updates by itself, ignoring all the
/// updates that arrive in the meantime. After all updates are fetched when this happens, the
/// client will resume normal operation, and the timeout will reset.
///
/// Documentation recommends 15 minutes without updates (https://core.telegram.org/api/updates).
pub(super) const NO_UPDATES_TIMEOUT: Duration = Duration::from_secs(15 * 60);

/// A [`MessageBox`] entry key.
#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum Entry {
    /// Account-wide `pts`.
    ///
    /// This includes private conversations (one-to-one) and small group chats.
    AccountWide,
    /// Account-wide `qts`.
    ///
    /// This includes only "secret" one-to-one chats.
    SecretChats,
    /// Channel-specific `pts`.
    ///
    /// This includes "megagroup", "broadcast" and "supergroup" channels.
    Channel(i64),
}

/// Represents a "message box" (event `pts` for a specific entry).
///
/// See <https://core.telegram.org/api/updates#message-related-event-sequences>.
#[derive(Debug)]
pub struct MessageBox {
    /// Map each entry to their current state.
    pub(super) map: HashMap<Entry, State>,

    // Additional fields beyond PTS needed by `Entry::AccountWide`.
    pub(super) date: i32,
    pub(super) seq: i32,

    /// Which entries have a gap and may soon trigger a need to get difference.
    ///
    /// If a gap is found, stores the required information to resolve it (when should it timeout and what updates
    /// should be held in case the gap is resolved on its own).
    ///
    /// Not stored directly in `map` as an optimization (else we would need another way of knowing which entries have
    /// a gap in them).
    pub(super) possible_gaps: HashMap<Entry, PossibleGap>,

    /// For which entries are we currently getting difference.
    pub(super) getting_diff_for: HashSet<Entry>,

    /// Holds the entry with the closest deadline.
    /// This field is merely an optimization, to avoid recalculating the closest deadline.
    pub(super) next_deadline: Option<Entry>,

    /// This field is merely an optimization, to reuse the same allocation.
    pub(super) tmp_entries: HashSet<Entry>,
}

/// Represents the information needed to correctly handle a specific `tl::enums::Update`.
#[derive(Debug)]
pub(super) struct PtsInfo {
    pub(super) pts: i32,
    pub(super) pts_count: i32,
    pub(super) entry: Entry,
}

/// The state of a particular entry in the message box.
#[derive(Debug)]
pub(super) struct State {
    /// Current local persistent timestamp.
    pub(super) pts: i32,

    /// Next instant when we would get the update difference if no updates arrived before then.
    pub(super) deadline: Instant,
}

// > ### Recovering gaps
// > […] Manually obtaining updates is also required in the following situations:
// > • Loss of sync: a gap was found in `seq` / `pts` / `qts` (as described above).
// >   It may be useful to wait up to 0.5 seconds in this situation and abort the sync in case a new update
// >   arrives, that fills the gap.
//
// This is really easy to trigger by spamming messages in a channel (with as little as 3 members works), because
// the updates produced by the RPC request take a while to arrive (whereas the read update comes faster alone).
#[derive(Debug)]
pub(super) struct PossibleGap {
    pub(super) deadline: Instant,
    /// Pending updates (those with a larger PTS, producing the gap which may later be filled).
    pub(super) updates: Vec<tl::enums::Update>,
}

#[derive(Debug)]
pub struct Gap;

/// Alias for the commonly-referenced three-tuple of update and related peers.
pub(super) type UpdateAndPeers = (
    Vec<tl::enums::Update>,
    Vec<tl::enums::User>,
    Vec<tl::enums::Chat>,
);
