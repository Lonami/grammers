// Copyright 2020 - developers of the `grammers` project.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// https://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or https://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.
use grammers_tl_types as tl;
use std::cmp::Ordering;
use std::cmp::Reverse;
use std::collections::BinaryHeap;
use std::collections::{HashMap, HashSet};
use tokio::time::{Duration, Instant};

/// Special made-up value to represent "there is no sequence" in `updatesCombined`.
const NO_SEQ: i32 = -1;

/// After how long without updates the client will "timeout".
///
/// When this timeout occurs, the client will attempt to fetch updates by itself, ignoring all the
/// updates that arrive in the meantime. After all updates are fetched when this happens, the
/// client will resume normal operation, and the timeout will reset.
///
/// Documentation recommends 15 minutes without updates (https://core.telegram.org/api/updates).
const NO_UPDATES_TIMEOUT: Duration = Duration::from_secs(15 * 60);

/// A [`MessageBox`] entry key.
#[derive(Clone, Copy, Hash, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum Entry {
    /// Account-wide `pts`.
    AccountWide,
    /// Account-wide `qts`.
    SecretChats,
    /// Channel-specific `pts`.
    Channel(i32),
}

/// Represents an action for how a certain update should be handled.
///
/// See https://core.telegram.org/api/updates#update-handling.
pub(crate) enum Action {
    /// This update should be applied (given to the client).
    Apply(tl::enums::Update),
    /// This update was already given to the client and should be skipped.
    Ignore,
    /// Some updates not received and there is a gap between the last known update and this one.
    /// The difference from the previously-known state and the current one should be fetched.
    GetDifference,
    // TODO handle all cases https://core.telegram.org/api/updates#recovering-gaps
}

/// Represents a "message box" (event `pts` for a specific entry).
///
/// See https://core.telegram.org/api/updates#message-related-event-sequences.
pub(crate) struct MessageBox {
    getting_diff: bool,
    deadline: Instant,
    date: i32,
    seq: i32,
    pts_map: HashMap<Entry, i32>,
}

/// Represents the information needed to correctly handle a specific `tl::enums::Update`.
struct PtsInfo {
    pts: i32,
    pts_count: i32,
    entry: Entry,
}

fn next_updates_deadline() -> Instant {
    Instant::now() + NO_UPDATES_TIMEOUT
}

fn handle_updates(updates: tl::types::Updates) -> tl::types::UpdatesCombined {
    tl::types::UpdatesCombined {
        updates: updates.updates,
        users: updates.users,
        chats: updates.chats,
        date: updates.date,
        seq_start: updates.seq,
        seq: updates.seq,
    }
}

fn handle_update_short(short: tl::types::UpdateShort) -> tl::types::UpdatesCombined {
    tl::types::UpdatesCombined {
        updates: vec![short.update],
        users: Vec::new(),
        chats: Vec::new(),
        date: short.date,
        seq_start: NO_SEQ,
        seq: NO_SEQ,
    }
}

fn handle_update_short_message(short: tl::types::UpdateShortMessage) -> tl::types::UpdatesCombined {
    handle_update_short(tl::types::UpdateShort {
        update: tl::types::UpdateNewMessage {
            message: tl::types::Message {
                out: short.out,
                mentioned: short.mentioned,
                media_unread: short.media_unread,
                silent: short.silent,
                post: false,
                from_scheduled: false,
                legacy: false,
                edit_hide: false,
                pinned: false,
                id: short.id,
                from_id: Some(
                    tl::types::PeerUser {
                        user_id: short.user_id,
                    }
                    .into(),
                ),
                // TODO this is wrong, it has to be ourself if it's outgoing
                peer_id: tl::types::PeerChat {
                    chat_id: short.user_id,
                }
                .into(),
                fwd_from: short.fwd_from,
                via_bot_id: short.via_bot_id,
                reply_to: short.reply_to,
                date: short.date,
                message: short.message,
                media: None,
                reply_markup: None,
                entities: short.entities,
                views: None,
                forwards: None,
                replies: None,
                edit_date: None,
                post_author: None,
                grouped_id: None,
                restriction_reason: None,
            }
            .into(),
            pts: short.pts,
            pts_count: short.pts_count,
        }
        .into(),
        date: short.date,
    })
}

fn handle_update_short_chat_message(
    short: tl::types::UpdateShortChatMessage,
) -> tl::types::UpdatesCombined {
    handle_update_short(tl::types::UpdateShort {
        update: tl::types::UpdateNewMessage {
            message: tl::types::Message {
                out: short.out,
                mentioned: short.mentioned,
                media_unread: short.media_unread,
                silent: short.silent,
                post: false,
                from_scheduled: false,
                legacy: false,
                edit_hide: false,
                pinned: false,
                id: short.id,
                from_id: Some(
                    tl::types::PeerUser {
                        user_id: short.from_id,
                    }
                    .into(),
                ),
                peer_id: tl::types::PeerChat {
                    chat_id: short.chat_id,
                }
                .into(),
                fwd_from: short.fwd_from,
                via_bot_id: short.via_bot_id,
                reply_to: short.reply_to,
                date: short.date,
                message: short.message,
                media: None,
                reply_markup: None,
                entities: short.entities,
                views: None,
                forwards: None,
                replies: None,
                edit_date: None,
                post_author: None,
                grouped_id: None,
                restriction_reason: None,
            }
            .into(),
            pts: short.pts,
            pts_count: short.pts_count,
        }
        .into(),
        date: short.date,
    })
}

impl MessageBox {
    pub(crate) fn new() -> Self {
        Self::from_pts(&[])
    }

    pub(crate) fn from_pts(entries_pts: &[(Entry, i32)]) -> Self {
        MessageBox {
            getting_diff: false,
            deadline: next_updates_deadline(),
            date: 0,
            seq: 0,
            pts_map: entries_pts.iter().copied().collect(),
        }
    }

    /// Process an update and return what should be done with it.
    pub(crate) fn process_updates(&mut self, updates: tl::enums::Updates) {
        self.deadline = next_updates_deadline();

        // > Implementations [have] to postpone updates received via the socket while filling
        // > gaps in the event and `Update` sequences, as well as avoid filling gaps in the same
        // > sequence.
        if self.getting_diff {
            return;
        }

        // Top level, when handling received `updates` and `updatesCombined`.
        // `updatesCombined` groups all the fields we care about, which is why we use it.
        let tl::types::UpdatesCombined { date, seq_start, seq, updates, users, chats } = match updates {
            // > `updatesTooLong` indicates that there are too many events pending to be pushed
            // > to the client, so one needs to fetch them manually.
            tl::enums::Updates::TooLong => {
                self.getting_diff = true;
                return;
            },
            // > `updateShortMessage`, `updateShortSentMessage` and `updateShortChatMessage` [...]
            // > should be transformed to `updateShort` upon receiving.
            tl::enums::Updates::UpdateShortMessage(short) => handle_update_short_message(short),
            tl::enums::Updates::UpdateShortChatMessage(short) => handle_update_short_chat_message(short),
            // > `updateShort` […] have lower priority and are broadcast to a large number of users.
            tl::enums::Updates::UpdateShort(short) => handle_update_short(short),
            // > [the] `seq` attribute, which indicates the remote `Updates` state after the
            // > generation of the `Updates`, and `seq_start` indicates the remote `Updates` state
            // > after the first of the `Updates` in the packet is generated
            tl::enums::Updates::Combined(combined) => combined,
            // > [the] `seq_start` attribute is omitted, because it is assumed that it is always
            // > equal to `seq`.
            tl::enums::Updates::Updates(updates) => handle_updates(updates),
            // Without the request `updateShortSentMessage` actually lacks fields like `message`,
            // which means it cannot be constructed on our own.
            tl::enums::Updates::UpdateShortSentMessage(_) => panic!("updateShortSentMessage can only be converted into updateShort by the caller of the request"),
        };

        // > For all the other [not `updates` or `updatesCombined`] `Updates` type constructors
        // > there is no need to check `seq` or change a local state.
        if seq_start != NO_SEQ {
            match (self.seq + 1).cmp(&seq_start) {
                // Apply
                Ordering::Equal => {}
                // Ignore
                Ordering::Greater => return,
                Ordering::Less => {
                    self.getting_diff = true;
                    return;
                }
            }

            self.date = date;
            if seq != NO_SEQ {
                self.seq = seq;
            }
        }

        for update in updates {
            if let Some(pts) = PtsInfo::from_update(&update) {
                if let Some(local_pts) = self.pts_map.get(pts.entry) {
                    match (local_pts + pts.pts_count).cmp(pts.pts) {
                        // Apply
                        Ordering::Equal => {}
                        // Ignore
                        Ordering::Greater => continue,
                        Ordering::Less => {
                            self.getting_diff = true;
                            return;
                        }
                    }
                }

                // First time (no previous `pts`) or update that we have to apply, both change the
                // local `pts`.
                self.pts_map.insert(pts.entry, pts.pts);
            }

            todo!("hand update to user");
        }
    }

    /// Return the next deadline when receiving updates should timeout.
    ///
    /// When this deadline is met, it means that get difference needs to be called.
    pub(crate) fn timeout_deadline(&self) -> Instant {
        self.deadline
    }
}

fn message_peer(message: &tl::enums::Message) -> Option<tl::enums::Peer> {
    match message {
        tl::enums::Message::Empty(_) => None,
        tl::enums::Message::Message(m) => Some(m.peer_id.clone()),
        tl::enums::Message::Service(m) => Some(m.peer_id.clone()),
    }
}

impl PtsInfo {
    fn from_update(update: &tl::enums::Update) -> Option<Self> {
        use tl::enums::Update::*;
        match update {
            NewMessage(u) => {
                assert!(!matches!(
                    message_peer(&u.message),
                    Some(tl::enums::Peer::Channel(_))
                ));
                Some(Self {
                    pts: u.pts,
                    pts_count: u.pts_count,
                    entry: todo!(),
                })
            }
            MessageId(_) => None,
            DeleteMessages(u) => Some(Self {
                pts: u.pts,
                pts_count: u.pts_count,
                entry: Entry::AccountWide,
            }),
            UserTyping(_) => None,
            ChatUserTyping(_) => None,
            ChatParticipants(_) => None,
            UserStatus(_) => None,
            UserName(_) => None,
            UserPhoto(_) => None,
            NewEncryptedMessage(u) => Some(Self {
                pts: u.qts,
                pts_count: 1,
                entry: Entry::SecretChats,
            }),
            EncryptedChatTyping(_) => None,
            Encryption(_) => None,
            EncryptedMessagesRead(_) => None,
            ChatParticipantAdd(_) => None,
            ChatParticipantDelete(_) => None,
            DcOptions(_) => None,
            NotifySettings(_) => None,
            ServiceNotification(_) => None,
            Privacy(_) => None,
            UserPhone(_) => None,
            ReadHistoryInbox(u) => {
                assert!(!matches!(u.peer, tl::enums::Peer::Channel(_)));
                Some(Self {
                    pts: u.pts,
                    pts_count: u.pts_count,
                    entry: Entry::AccountWide,
                })
            }
            ReadHistoryOutbox(u) => {
                assert!(!matches!(u.peer, tl::enums::Peer::Channel(_)));
                Some(Self {
                    pts: u.pts,
                    pts_count: u.pts_count,
                    entry: Entry::AccountWide,
                })
            }
            WebPage(u) => Some(Self {
                pts: u.pts,
                pts_count: u.pts_count,
                entry: Entry::AccountWide,
            }),
            ReadMessagesContents(u) => Some(Self {
                pts: u.pts,
                pts_count: u.pts_count,
                entry: Entry::AccountWide,
            }),
            ChannelTooLong(u) => u.pts.map(|pts| Self {
                pts,
                pts_count: 1,
                entry: Entry::Channel(u.channel_id),
            }),
            Channel(_) => None,
            NewChannelMessage(u) => Some(Self {
                pts: u.pts,
                pts_count: u.pts_count,
                entry: todo!(),
            }),
            ReadChannelInbox(u) => Some(Self {
                pts: u.pts,
                pts_count: 1,
                entry: Entry::Channel(u.channel_id),
            }),
            DeleteChannelMessages(u) => Some(Self {
                pts: u.pts,
                pts_count: u.pts_count,
                entry: Entry::Channel(u.channel_id),
            }),
            ChannelMessageViews(_) => None,
            ChatParticipantAdmin(_) => None,
            NewStickerSet(_) => None,
            StickerSetsOrder(_) => None,
            StickerSets => None,
            SavedGifs => None,
            BotInlineQuery(_) => None,
            BotInlineSend(_) => None,
            EditChannelMessage(u) => Some(Self {
                pts: u.pts,
                pts_count: u.pts_count,
                entry: todo!(),
            }),
            BotCallbackQuery(_) => None,
            EditMessage(u) => {
                assert!(!matches!(
                    message_peer(&u.message),
                    Some(tl::enums::Peer::Channel(_))
                ));
                Some(Self {
                    pts: u.pts,
                    pts_count: u.pts_count,
                    entry: Entry::AccountWide,
                })
            }
            InlineBotCallbackQuery(_) => None,
            ReadChannelOutbox(_) => None,
            DraftMessage(_) => None,
            ReadFeaturedStickers => None,
            RecentStickers => None,
            Config => None,
            PtsChanged => None,
            ChannelWebPage(u) => Some(Self {
                pts: u.pts,
                pts_count: u.pts_count,
                entry: Entry::Channel(u.channel_id),
            }),
            DialogPinned(_) => None,
            PinnedDialogs(_) => None,
            BotWebhookJson(_) => None,
            BotWebhookJsonquery(_) => None,
            BotShippingQuery(_) => None,
            BotPrecheckoutQuery(_) => None,
            PhoneCall(_) => None,
            LangPackTooLong(_) => None,
            LangPack(_) => None,
            FavedStickers => None,
            ChannelReadMessagesContents(_) => None,
            ContactsReset => None,
            ChannelAvailableMessages(_) => None,
            DialogUnreadMark(_) => None,
            MessagePoll(_) => None,
            ChatDefaultBannedRights(_) => None,
            FolderPeers(u) => Some(Self {
                pts: u.pts,
                pts_count: u.pts_count,
                entry: Entry::AccountWide,
            }),
            PeerSettings(_) => None,
            PeerLocated(_) => None,
            NewScheduledMessage(_) => None,
            DeleteScheduledMessages(_) => None,
            Theme(_) => None,
            GeoLiveViewed(_) => None,
            LoginToken => None,
            MessagePollVote(_) => None,
            DialogFilter(_) => None,
            DialogFilterOrder(_) => None,
            DialogFilters => None,
            PhoneCallSignalingData(_) => None,
            ChannelParticipant(u) => Some(Self {
                pts: u.qts,
                pts_count: 1,
                entry: Entry::SecretChats,
            }),
            ChannelMessageForwards(_) => None,
            ReadChannelDiscussionInbox(_) => None,
            ReadChannelDiscussionOutbox(_) => None,
            PeerBlocked(_) => None,
            ChannelUserTyping(_) => None,
            PinnedMessages(u) => {
                assert!(!matches!(u.peer, tl::enums::Peer::Channel(_)));
                Some(Self {
                    pts: u.pts,
                    pts_count: u.pts_count,
                    entry: Entry::AccountWide,
                })
            }
            PinnedChannelMessages(u) => Some(Self {
                pts: u.pts,
                pts_count: u.pts_count,
                entry: Entry::Channel(u.channel_id),
            }),
        }
    }
}
