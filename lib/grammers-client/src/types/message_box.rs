// Copyright 2020 - developers of the `grammers` project.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// https://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or https://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.
use super::EntityCache;
use grammers_tl_types as tl;
use log::{debug, info, trace};
use std::cmp::Ordering;
use std::collections::{HashMap, HashSet};
use tokio::time::{Duration, Instant};

/// Telegram sends `seq` equal to `0` when "it doesn't matter", so we use that value too.
const NO_SEQ: i32 = 0;

// See https://core.telegram.org/method/updates.getChannelDifference.
const BOT_CHANNEL_DIFF_LIMIT: i32 = 100000;
const USER_CHANNEL_DIFF_LIMIT: i32 = 100;

/// After how long without updates the client will "timeout".
///
/// When this timeout occurs, the client will attempt to fetch updates by itself, ignoring all the
/// updates that arrive in the meantime. After all updates are fetched when this happens, the
/// client will resume normal operation, and the timeout will reset.
///
/// Documentation recommends 15 minutes without updates (https://core.telegram.org/api/updates).
const NO_UPDATES_TIMEOUT: Duration = Duration::from_secs(15 * 60);

/// A [`MessageBox`] entry key.
#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum Entry {
    /// Account-wide `pts`.
    AccountWide,
    /// Account-wide `qts`.
    SecretChats,
    /// Channel-specific `pts`.
    Channel(i32),
}

/// Represents a "message box" (event `pts` for a specific entry).
///
/// See https://core.telegram.org/api/updates#message-related-event-sequences.
#[derive(Debug)]
pub(crate) struct MessageBox {
    getting_diff: bool,
    getting_channel_diff: HashSet<i32>,
    deadline: Instant,
    date: i32,
    seq: i32,
    pts_map: HashMap<Entry, i32>,
}

/// Represents the information needed to correctly handle a specific `tl::enums::Update`.
#[derive(Debug)]
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

fn handle_update_short_message(
    short: tl::types::UpdateShortMessage,
    self_id: i32,
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
                        user_id: if short.out { self_id } else { short.user_id },
                    }
                    .into(),
                ),
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

fn handle_update_short_sent_message(
    short: tl::types::UpdateShortSentMessage,
) -> tl::types::UpdatesCombined {
    handle_update_short(tl::types::UpdateShort {
        update: tl::types::UpdateNewMessage {
            message: tl::types::MessageEmpty { id: short.id }.into(),
            pts: short.pts,
            pts_count: short.pts_count,
        }
        .into(),
        date: short.date,
    })
}

pub(crate) struct Gap;

impl MessageBox {
    pub(crate) fn new() -> Self {
        MessageBox {
            getting_diff: false,
            getting_channel_diff: HashSet::new(),
            deadline: next_updates_deadline(),
            date: 1,
            seq: 0,
            pts_map: HashMap::new(),
        }
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

    /// Return the request that needs to be made to get the difference, if any.
    pub(crate) fn get_difference(&self) -> Option<tl::functions::updates::GetDifference> {
        if self.getting_diff || Instant::now() > self.deadline {
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

    /// Return the request that needs to be made to get a channel's difference, if any.
    pub(crate) fn get_channel_difference(
        &mut self,
        entities: &EntityCache,
    ) -> Option<tl::functions::updates::GetChannelDifference> {
        let channel_id = *self.getting_channel_diff.iter().next()?;
        let channel = if let Some(channel) = entities.get_input_channel(channel_id) {
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
                limit: if entities.is_self_bot() {
                    BOT_CHANNEL_DIFF_LIMIT
                } else {
                    USER_CHANNEL_DIFF_LIMIT
                },
            })
        } else {
            self.getting_channel_diff.remove(&channel_id);
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
                // This `diff` has the "latest messages and corresponding entities", but it would
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

    /// Process an update and return what should be done with it.
    pub(crate) fn process_updates(
        &mut self,
        updates: tl::enums::Updates,
        entities: &EntityCache,
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
        } = match updates {
            // > `updatesTooLong` indicates that there are too many events pending to be pushed
            // > to the client, so one needs to fetch them manually.
            tl::enums::Updates::TooLong => {
                self.getting_diff = true;
                return Err(Gap);
            }
            // > `updateShortMessage`, `updateShortSentMessage` and `updateShortChatMessage` [...]
            // > should be transformed to `updateShort` upon receiving.
            tl::enums::Updates::UpdateShortMessage(short) => {
                // > Incomplete update: the client is missing data about a chat/user from one of
                // > the shortened constructors, such as `updateShortChatMessage`, etc.
                //
                // This only needs to be done for "short messages", to get the private chat (user)
                // where the message occured. Anywhere else, Telegram should send information
                // about the chat so that [min constructors][0] can be used.
                //
                // [0]: https://core.telegram.org/api/min
                if !entities.contains_user(short.user_id) {
                    info!("no hash for user {} known, treating as gap", short.user_id);
                    return Err(Gap);
                }
                handle_update_short_message(short, entities.self_id())
            }
            tl::enums::Updates::UpdateShortChatMessage(short) => {
                // No need to check for entities here. Chats do not require an access hash, and
                // min constructors can be used to access the user.
                handle_update_short_chat_message(short)
            }
            // > `updateShort` […] have lower priority and are broadcast to a large number of users.
            //
            // There *shouldn't* be updates mentioning peers we're unaware of here.
            //
            // If later it turns out these can happen, the code will need to be updated to
            // consider entities missing here a gap as well.
            tl::enums::Updates::UpdateShort(short) => handle_update_short(short),
            // > [the] `seq` attribute, which indicates the remote `Updates` state after the
            // > generation of the `Updates`, and `seq_start` indicates the remote `Updates` state
            // > after the first of the `Updates` in the packet is generated
            tl::enums::Updates::Combined(combined) => combined,
            // > [the] `seq_start` attribute is omitted, because it is assumed that it is always
            // > equal to `seq`.
            tl::enums::Updates::Updates(updates) => handle_updates(updates),
            // Even though we lack fields like the message text, it still needs to be handled, so
            // that the `pts` can be kept consistent.
            tl::enums::Updates::UpdateShortSentMessage(short) => {
                handle_update_short_sent_message(short)
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
                    info!(
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

        let mut result = Vec::with_capacity(updates.len());
        for update in updates {
            if let Some(pts) = PtsInfo::from_update(&update) {
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
                            continue;
                        }
                        Ordering::Less => {
                            info!(
                                "gap on update for {:?} (local {:?}, count {:?}, remote {:?})",
                                pts.entry, local_pts, pts.pts_count, pts.pts
                            );
                            self.getting_diff = true;
                            return Err(Gap);
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
            }

            result.push(update);
        }

        Ok((result, users, chats))
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

fn message_channel_id(message: &tl::enums::Message) -> Option<i32> {
    match message {
        tl::enums::Message::Empty(_) => None,
        tl::enums::Message::Message(m) => match &m.peer_id {
            tl::enums::Peer::Channel(c) => Some(c.channel_id),
            _ => None,
        },
        tl::enums::Message::Service(m) => match &m.peer_id {
            tl::enums::Peer::Channel(c) => Some(c.channel_id),
            _ => None,
        },
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
                    entry: Entry::AccountWide,
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
                pts_count: 0,
                entry: Entry::Channel(u.channel_id),
            }),
            Channel(_) => None,
            // Telegram actually sends `updateNewChannelMessage(messageEmpty(…))`, and because
            // there's no way to tell which channel ID this `pts` belongs to, the best we can
            // do is ignore it.
            //
            // Future messages should trigger a gap that we need to recover from.
            NewChannelMessage(u) => message_channel_id(&u.message).map(|channel_id| Self {
                pts: u.pts,
                pts_count: u.pts_count,
                entry: Entry::Channel(channel_id),
            }),
            ReadChannelInbox(u) => Some(Self {
                pts: u.pts,
                pts_count: 0,
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
            EditChannelMessage(u) => message_channel_id(&u.message).map(|channel_id| Self {
                pts: u.pts,
                pts_count: u.pts_count,
                entry: Entry::Channel(channel_id),
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
                pts_count: 0,
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
            Chat(_) => None,
            GroupCallParticipants(_) => None,
            GroupCall(_) => None,
        }
    }
}
