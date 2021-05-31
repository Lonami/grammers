// Copyright 2020 - developers of the `grammers` project.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// https://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or https://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.
use super::defs::{Entry, Gap, PtsInfo, NO_SEQ};
use super::ChatHashCache;
use grammers_tl_types as tl;
use log::info;

// > The `updateShortMessage`, `updateShortSentMessage` and `updateShortChatMessage` constructors
// > [...] should be transformed to `updateShort` upon receiving.
//
// We don't use `updateShort` because it's fairly limitting. `updatesCombined` is big enough to
// contain all updates inside, and sticking to a single type (and not enum) simplifies things.
//
// This module's job is just converting the various updates types into `updatesCombined`.
// It also converts updates into their corresponding `PtsInfo`.

pub(super) fn updates(updates: tl::types::Updates) -> tl::types::UpdatesCombined {
    tl::types::UpdatesCombined {
        updates: updates.updates,
        users: updates.users,
        chats: updates.chats,
        date: updates.date,
        seq_start: updates.seq,
        seq: updates.seq,
    }
}

pub(super) fn update_short(short: tl::types::UpdateShort) -> tl::types::UpdatesCombined {
    tl::types::UpdatesCombined {
        updates: vec![short.update],
        users: Vec::new(),
        chats: Vec::new(),
        date: short.date,
        seq_start: NO_SEQ,
        seq: NO_SEQ,
    }
}

pub(super) fn update_short_message(
    short: tl::types::UpdateShortMessage,
    self_id: i32,
) -> tl::types::UpdatesCombined {
    update_short(tl::types::UpdateShort {
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
                ttl_period: short.ttl_period,
            }
            .into(),
            pts: short.pts,
            pts_count: short.pts_count,
        }
        .into(),
        date: short.date,
    })
}

pub(super) fn update_short_chat_message(
    short: tl::types::UpdateShortChatMessage,
) -> tl::types::UpdatesCombined {
    update_short(tl::types::UpdateShort {
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
                ttl_period: short.ttl_period,
            }
            .into(),
            pts: short.pts,
            pts_count: short.pts_count,
        }
        .into(),
        date: short.date,
    })
}

pub(super) fn update_short_sent_message(
    short: tl::types::UpdateShortSentMessage,
) -> tl::types::UpdatesCombined {
    update_short(tl::types::UpdateShort {
        update: tl::types::UpdateNewMessage {
            message: tl::types::MessageEmpty {
                id: short.id,
                peer_id: None,
            }
            .into(),
            pts: short.pts,
            pts_count: short.pts_count,
        }
        .into(),
        date: short.date,
    })
}

pub(super) fn adapt(
    updates: tl::enums::Updates,
    chat_hashes: &mut ChatHashCache,
) -> Result<tl::types::UpdatesCombined, Gap> {
    Ok(match updates {
        // > `updatesTooLong` indicates that there are too many events pending to be pushed
        // > to the client, so one needs to fetch them manually.
        tl::enums::Updates::TooLong => {
            info!("received updatesTooLong, treating as gap");
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
            if !chat_hashes.contains_user(short.user_id) {
                info!("no hash for user {} known, treating as gap", short.user_id);
                return Err(Gap);
            }
            update_short_message(short, chat_hashes.self_id())
        }
        tl::enums::Updates::UpdateShortChatMessage(short) => {
            // No need to check for chats here. Small group chats do not require an access
            // hash, and min constructors can be used to access the user.
            update_short_chat_message(short)
        }
        // > `updateShort` […] have lower priority and are broadcast to a large number of users.
        //
        // There *shouldn't* be updates mentioning peers we're unaware of here.
        //
        // If later it turns out these can happen, the code will need to be updated to
        // consider chats missing here a gap as well.
        tl::enums::Updates::UpdateShort(short) => update_short(short),
        // > [the] `seq` attribute, which indicates the remote `Updates` state after the
        // > generation of the `Updates`, and `seq_start` indicates the remote `Updates` state
        // > after the first of the `Updates` in the packet is generated
        tl::enums::Updates::Combined(combined) => {
            chat_hashes.extend(&combined.users, &combined.chats);
            combined
        }
        // > [the] `seq_start` attribute is omitted, because it is assumed that it is always
        // > equal to `seq`.
        tl::enums::Updates::Updates(updates) => {
            chat_hashes.extend(&updates.users, &updates.chats);
            self::updates(updates)
        }
        // Even though we lack fields like the message text, it still needs to be handled, so
        // that the `pts` can be kept consistent.
        tl::enums::Updates::UpdateShortSentMessage(short) => update_short_sent_message(short),
    })
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
    pub(super) fn from_update(update: &tl::enums::Update) -> Option<Self> {
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
            PeerHistoryTtl(_) => None,
            ChatParticipant(u) => Some(Self {
                pts: u.qts,
                pts_count: 0,
                entry: Entry::SecretChats,
            }),
            ChannelParticipant(u) => Some(Self {
                pts: u.qts,
                pts_count: 0,
                entry: Entry::SecretChats,
            }),
            BotStopped(u) => Some(Self {
                pts: u.qts,
                pts_count: 0,
                entry: Entry::SecretChats,
            }),
        }
    }
}
