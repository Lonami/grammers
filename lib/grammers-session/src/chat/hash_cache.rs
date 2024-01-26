// Copyright 2020 - developers of the `grammers` project.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// https://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or https://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.
use super::{PackedChat, PackedType};
use grammers_tl_types as tl;
use std::collections::HashMap;

/// In-memory chat cache, mapping peers to their respective access hashes.
pub struct ChatHashCache {
    // As far as I've observed, user, chat and channel IDs cannot collide,
    // but it will be an interesting moment if they ever do.
    hash_map: HashMap<i64, (i64, PackedType)>,
    self_id: Option<i64>,
    self_bot: bool,
}

impl ChatHashCache {
    pub fn new(self_user: Option<(i64, bool)>) -> Self {
        Self {
            hash_map: HashMap::new(),
            self_id: self_user.map(|user| user.0),
            self_bot: self_user.map(|user| user.1).unwrap_or(false),
        }
    }

    pub fn self_id(&self) -> i64 {
        self.self_id
            .expect("tried to query self_id before it's known")
    }

    pub fn is_self_bot(&self) -> bool {
        self.self_bot
    }

    pub fn set_self_user(&mut self, user: PackedChat) {
        self.self_bot = match user.ty {
            PackedType::User => false,
            PackedType::Bot => true,
            _ => panic!("tried to set self-user without providing user type"),
        };
        self.self_id = Some(user.id);
    }

    pub fn get(&self, id: i64) -> Option<PackedChat> {
        self.hash_map.get(&id).map(|&(hash, ty)| PackedChat {
            ty,
            id,
            access_hash: Some(hash),
        })
    }

    #[inline]
    fn has(&self, id: i64) -> bool {
        self.hash_map.contains_key(&id)
    }

    fn has_peer(&self, peer: &tl::enums::Peer) -> bool {
        match peer {
            tl::enums::Peer::User(user) => self.has(user.user_id),
            tl::enums::Peer::Chat(_chat) => true, // no hash needed, so we always have it
            tl::enums::Peer::Channel(channel) => self.has(channel.channel_id),
        }
    }

    fn has_dialog_peer(&self, peer: &tl::enums::DialogPeer) -> bool {
        match peer {
            tl::enums::DialogPeer::Peer(p) => self.has_peer(&p.peer),
            tl::enums::DialogPeer::Folder(_) => true,
        }
    }

    fn has_user(&self, peer: &tl::enums::InputUser) -> bool {
        match peer {
            tl::enums::InputUser::Empty => true,
            tl::enums::InputUser::UserSelf => true,
            tl::enums::InputUser::User(user) => self.has(user.user_id),
            tl::enums::InputUser::FromMessage(message) => self.has(message.user_id),
        }
    }

    fn has_participant(&self, participant: &tl::enums::ChatParticipant) -> bool {
        match participant {
            tl::enums::ChatParticipant::Participant(p) => {
                self.has(p.user_id) && self.has(p.inviter_id)
            }
            tl::enums::ChatParticipant::Creator(p) => self.has(p.user_id),
            tl::enums::ChatParticipant::Admin(p) => self.has(p.user_id) && self.has(p.inviter_id),
        }
    }

    fn has_channel_participant(&self, participant: &tl::enums::ChannelParticipant) -> bool {
        match participant {
            tl::enums::ChannelParticipant::Participant(p) => self.has(p.user_id),
            tl::enums::ChannelParticipant::ParticipantSelf(p) => {
                self.has(p.user_id) && self.has(p.inviter_id)
            }
            tl::enums::ChannelParticipant::Creator(p) => self.has(p.user_id),
            tl::enums::ChannelParticipant::Admin(p) => {
                self.has(p.user_id)
                    && match p.inviter_id {
                        Some(i) => self.has(i),
                        None => true,
                    }
                    && self.has(p.promoted_by)
            }
            tl::enums::ChannelParticipant::Banned(p) => {
                self.has_peer(&p.peer) && self.has(p.kicked_by)
            }
            tl::enums::ChannelParticipant::Left(p) => self.has_peer(&p.peer),
        }
    }

    // Returns `true` if all users and chats could be extended without issue.
    // Returns `false` if there is any user or chat for which its `access_hash` is missing.
    #[must_use]
    pub fn extend(&mut self, users: &[tl::enums::User], chats: &[tl::enums::Chat]) -> bool {
        // See https://core.telegram.org/api/min for "issues" with "min constructors".
        use tl::enums::{Chat as C, User as U};

        let mut success = true;

        users.iter().for_each(|user| match user {
            U::Empty(_) => {}
            U::User(u) => match (u.min, u.access_hash) {
                (false, Some(hash)) => {
                    let ty = if u.bot {
                        PackedType::Bot
                    } else {
                        PackedType::User
                    };
                    self.hash_map.insert(u.id, (hash, ty));
                }
                _ => success &= self.hash_map.contains_key(&u.id),
            },
        });

        chats.iter().for_each(|chat| match chat {
            C::Empty(_) | C::Chat(_) | C::Forbidden(_) => {}
            C::Channel(c) => match (c.min, c.access_hash) {
                (false, Some(hash)) => {
                    let ty = if c.megagroup {
                        PackedType::Megagroup
                    } else if c.gigagroup {
                        PackedType::Gigagroup
                    } else {
                        PackedType::Broadcast
                    };
                    self.hash_map.insert(c.id, (hash, ty));
                }
                _ => success &= self.hash_map.contains_key(&c.id),
            },
            C::ChannelForbidden(c) => {
                let ty = if c.megagroup {
                    PackedType::Megagroup
                } else {
                    PackedType::Broadcast
                };
                self.hash_map.insert(c.id, (c.access_hash, ty));
            }
        });

        success
    }

    // Like `Self::extend`, but intended for socket updates.
    pub fn extend_from_updates(&mut self, updates: &tl::enums::Updates) -> bool {
        use tl::enums::Update as U;

        match updates {
            tl::enums::Updates::TooLong => true,
            tl::enums::Updates::UpdateShortMessage(short) => self.has(short.user_id),
            tl::enums::Updates::UpdateShortChatMessage(short) => self.has(short.from_id),
            tl::enums::Updates::UpdateShort(short) => match &short.update {
                U::NewMessage(u) => self.extend_from_message(&u.message),
                U::MessageId(_) => true,
                U::DeleteMessages(_) => true,
                U::UserTyping(u) => self.has(u.user_id),
                U::ChatUserTyping(u) => self.has_peer(&u.from_id),
                U::ChatParticipants(u) => match &u.participants {
                    tl::enums::ChatParticipants::Forbidden(_) => true,
                    tl::enums::ChatParticipants::Participants(c) => {
                        c.participants.iter().all(|p| self.has_participant(p))
                    }
                },
                U::UserStatus(u) => self.has(u.user_id),
                U::UserName(u) => self.has(u.user_id),
                U::NewAuthorization(_) => true,
                U::NewEncryptedMessage(_) => true,
                U::EncryptedChatTyping(_) => true,
                U::Encryption(_) => true,
                U::EncryptedMessagesRead(_) => true,
                U::ChatParticipantAdd(u) => self.has(u.user_id) && self.has(u.inviter_id),
                U::ChatParticipantDelete(u) => self.has(u.user_id),
                U::DcOptions(_) => true,
                U::NotifySettings(u) => match &u.peer {
                    tl::enums::NotifyPeer::Peer(n) => self.has_peer(&n.peer),
                    tl::enums::NotifyPeer::NotifyForumTopic(n) => self.has_peer(&n.peer),
                    tl::enums::NotifyPeer::NotifyUsers
                    | tl::enums::NotifyPeer::NotifyChats
                    | tl::enums::NotifyPeer::NotifyBroadcasts => true,
                },
                U::ServiceNotification(_) => true,
                U::Privacy(_) => true,
                U::UserPhone(u) => self.has(u.user_id),
                U::ReadHistoryInbox(u) => self.has_peer(&u.peer),
                U::ReadHistoryOutbox(u) => self.has_peer(&u.peer),
                U::WebPage(_) => true,
                U::ReadMessagesContents(_) => true,
                U::ChannelTooLong(u) => self.has(u.channel_id),
                U::Channel(u) => self.has(u.channel_id),
                U::NewChannelMessage(u) => self.extend_from_message(&u.message),
                U::ReadChannelInbox(u) => self.has(u.channel_id),
                U::DeleteChannelMessages(u) => self.has(u.channel_id),
                U::ChannelMessageViews(u) => self.has(u.channel_id),
                U::ChatParticipantAdmin(u) => self.has(u.user_id),
                U::NewStickerSet(_) => true,
                U::StickerSetsOrder(_) => true,
                U::StickerSets(_) => true,
                U::SavedGifs => true,
                U::BotInlineQuery(u) => self.has(u.user_id),
                U::BotInlineSend(u) => self.has(u.user_id),
                U::EditChannelMessage(u) => self.extend_from_message(&u.message),
                U::BotCallbackQuery(u) => self.has(u.user_id),
                U::EditMessage(u) => self.extend_from_message(&u.message),
                U::InlineBotCallbackQuery(u) => self.has(u.user_id),
                U::ReadChannelOutbox(u) => self.has(u.channel_id),
                U::DraftMessage(u) => self.has_peer(&u.peer),
                U::ReadFeaturedStickers => true,
                U::RecentStickers => true,
                U::Config => true,
                U::PtsChanged => true,
                U::ChannelWebPage(u) => self.has(u.channel_id),
                U::DialogPinned(u) => self.has_dialog_peer(&u.peer),
                U::PinnedDialogs(u) => match &u.order {
                    Some(o) => o.iter().all(|d| self.has_dialog_peer(d)),
                    None => true,
                },
                U::BotWebhookJson(_) => true,
                U::BotWebhookJsonquery(_) => true,
                U::BotShippingQuery(u) => self.has(u.user_id),
                U::BotPrecheckoutQuery(u) => self.has(u.user_id),
                U::PhoneCall(_) => true,
                U::LangPackTooLong(_) => true,
                U::LangPack(_) => true,
                U::FavedStickers => true,
                U::ChannelReadMessagesContents(u) => self.has(u.channel_id),
                U::ContactsReset => true,
                U::ChannelAvailableMessages(u) => self.has(u.channel_id),
                U::DialogUnreadMark(u) => self.has_dialog_peer(&u.peer),
                U::MessagePoll(_) => true,
                U::ChatDefaultBannedRights(u) => self.has_peer(&u.peer),
                U::FolderPeers(u) => u.folder_peers.iter().all(|f| match f {
                    tl::enums::FolderPeer::Peer(p) => self.has_peer(&p.peer),
                }),
                U::PeerSettings(u) => self.has_peer(&u.peer),
                U::PeerLocated(u) => u.peers.iter().all(|p| match p {
                    tl::enums::PeerLocated::Located(l) => self.has_peer(&l.peer),
                    tl::enums::PeerLocated::PeerSelfLocated(_) => true,
                }),
                U::NewScheduledMessage(u) => self.extend_from_message(&u.message),
                U::DeleteScheduledMessages(u) => self.has_peer(&u.peer),
                U::Theme(_) => true,
                U::GeoLiveViewed(u) => self.has_peer(&u.peer),
                U::LoginToken => true,
                U::MessagePollVote(u) => self.has_peer(&u.peer),
                U::DialogFilter(_) => true,
                U::DialogFilterOrder(_) => true,
                U::DialogFilters => true,
                U::PhoneCallSignalingData(_) => true,
                U::ChannelMessageForwards(u) => self.has(u.channel_id),
                U::ReadChannelDiscussionInbox(u) => self.has(u.channel_id),
                U::ReadChannelDiscussionOutbox(u) => self.has(u.channel_id),
                U::PeerBlocked(u) => self.has_peer(&u.peer_id),
                U::ChannelUserTyping(u) => self.has(u.channel_id) && self.has_peer(&u.from_id),
                U::PinnedMessages(u) => self.has_peer(&u.peer),
                U::PinnedChannelMessages(u) => self.has(u.channel_id),
                U::Chat(_) => true,
                U::GroupCallParticipants(u) => u.participants.iter().all(|p| match p {
                    tl::enums::GroupCallParticipant::Participant(p) => self.has_peer(&p.peer),
                }),
                U::GroupCall(_) => true,
                U::PeerHistoryTtl(u) => self.has_peer(&u.peer),
                U::ChatParticipant(u) => {
                    self.has(u.actor_id)
                        && self.has(u.user_id)
                        && match &u.prev_participant {
                            Some(p) => self.has_participant(p),
                            None => true,
                        }
                        && match &u.new_participant {
                            Some(p) => self.has_participant(p),
                            None => true,
                        }
                        && match &u.invite {
                            Some(tl::enums::ExportedChatInvite::ChatInviteExported(e)) => {
                                self.has(e.admin_id)
                            }
                            Some(tl::enums::ExportedChatInvite::ChatInvitePublicJoinRequests)
                            | None => true,
                        }
                }
                U::ChannelParticipant(u) => {
                    self.has(u.channel_id)
                        && self.has(u.actor_id)
                        && self.has(u.user_id)
                        && match &u.prev_participant {
                            Some(p) => self.has_channel_participant(p),
                            None => true,
                        }
                        && match &u.new_participant {
                            Some(p) => self.has_channel_participant(p),
                            None => true,
                        }
                        && match &u.invite {
                            Some(tl::enums::ExportedChatInvite::ChatInviteExported(e)) => {
                                self.has(e.admin_id)
                            }
                            Some(tl::enums::ExportedChatInvite::ChatInvitePublicJoinRequests)
                            | None => true,
                        }
                }
                U::BotStopped(u) => self.has(u.user_id),
                U::GroupCallConnection(_) => true,
                U::BotCommands(u) => self.has_peer(&u.peer) && self.has(u.bot_id),
                U::PendingJoinRequests(u) => self.has_peer(&u.peer),
                U::BotChatInviteRequester(u) => {
                    self.has_peer(&u.peer)
                        && self.has(u.user_id)
                        && match &u.invite {
                            tl::enums::ExportedChatInvite::ChatInviteExported(e) => {
                                self.has(e.admin_id)
                            }
                            tl::enums::ExportedChatInvite::ChatInvitePublicJoinRequests => true,
                        }
                }
                U::MessageReactions(u) => self.has_peer(&u.peer),
                U::AttachMenuBots => true,
                U::WebViewResultSent(_) => true,
                U::BotMenuButton(u) => self.has(u.bot_id),
                U::SavedRingtones => true,
                U::TranscribedAudio(u) => self.has_peer(&u.peer),
                U::ReadFeaturedEmojiStickers => true,
                U::UserEmojiStatus(u) => self.has(u.user_id),
                U::RecentEmojiStatuses => true,
                U::RecentReactions => true,
                U::MoveStickerSetToTop(_) => true,
                U::MessageExtendedMedia(u) => self.has_peer(&u.peer),
                U::ChannelPinnedTopic(u) => self.has(u.channel_id),
                U::ChannelPinnedTopics(u) => self.has(u.channel_id),
                U::User(u) => self.has(u.user_id),
                U::AutoSaveSettings => true,
                U::GroupInvitePrivacyForbidden(u) => self.has(u.user_id),
                U::Story(u) => self.has_peer(&u.peer),
                U::ReadStories(u) => self.has_peer(&u.peer),
                U::StoryId(_) => true,
                U::StoriesStealthMode(_) => true,
                U::SentStoryReaction(u) => self.has_peer(&u.peer),
                U::BotChatBoost(u) => self.has_peer(&u.peer),
                U::ChannelViewForumAsMessages(u) => self.has(u.channel_id),
                U::PeerWallpaper(u) => self.has_peer(&u.peer),
                U::BotMessageReaction(u) => self.has_peer(&u.peer),
                U::BotMessageReactions(u) => self.has_peer(&u.peer),
                U::SavedDialogPinned(u) => self.has_dialog_peer(&u.peer),
                U::PinnedSavedDialogs(_) => true,
                U::SavedReactionTags => true,
            },
            // Telegram should be including all the peers referenced in the updates in
            // `.users` and `.chats`, so no instrospection is done (unlike for `UpdateShort`).
            //
            // If it turns out that there is some peer somewhere within the updates which was not
            // actually known, the solution would be to check all inner updates in the same way
            // `UpdateShort` is checked (which is sadly quite wasteful).
            tl::enums::Updates::Combined(combined) => self.extend(&combined.users, &combined.chats),
            tl::enums::Updates::Updates(updates) => self.extend(&updates.users, &updates.chats),
            tl::enums::Updates::UpdateShortSentMessage(_short) => true,
        }
    }

    // Like `Self::extend`, but intended for a message.
    // It won't actually extend, because it can't, but it will make sure the hash is known.
    fn extend_from_message(&mut self, message: &tl::enums::Message) -> bool {
        use tl::enums::MessageAction as MA;
        use tl::enums::MessageReplyHeader as MRH;

        match message {
            tl::enums::Message::Empty(m) => match &m.peer_id {
                Some(p) => self.has_peer(p),
                None => true,
            },
            tl::enums::Message::Message(m) => {
                // The parenthesis are needed: https://github.com/rust-lang/rust/issues/101234
                (match &m.from_id {
                    Some(p) => self.has_peer(p),
                    None => true,
                }) && self.has_peer(&m.peer_id)
                    && match &m.fwd_from {
                        Some(tl::enums::MessageFwdHeader::Header(f)) => {
                            (match &f.from_id {
                                Some(p) => self.has_peer(p),
                                None => true,
                            }) && match &f.saved_from_peer {
                                Some(p) => self.has_peer(p),
                                None => true,
                            }
                        }
                        None => true,
                    }
                    && match &m.reply_to {
                        Some(MRH::Header(r)) => match &r.reply_to_peer_id {
                            Some(p) => self.has_peer(p),
                            None => true,
                        },
                        Some(MRH::MessageReplyStoryHeader(r)) => self.has(r.user_id),
                        None => true,
                    }
                    && match &m.reply_markup {
                        Some(tl::enums::ReplyMarkup::ReplyKeyboardHide(_)) => true,
                        Some(tl::enums::ReplyMarkup::ReplyKeyboardForceReply(_)) => true,
                        Some(tl::enums::ReplyMarkup::ReplyKeyboardMarkup(r)) => {
                            r.rows.iter().all(|r| match r {
                                tl::enums::KeyboardButtonRow::Row(r) => r.buttons.iter().all(|b| {
                                    match b {
                                    tl::enums::KeyboardButton::InputKeyboardButtonUrlAuth(b) => {
                                        self.has_user(&b.bot)
                                    }
                                    tl::enums::KeyboardButton::InputKeyboardButtonUserProfile(
                                        b,
                                    ) => self.has_user(&b.user_id),
                                    tl::enums::KeyboardButton::UserProfile(b) => {
                                        self.has(b.user_id)
                                    }
                                    _ => true,
                                }
                                }),
                            })
                        }
                        Some(tl::enums::ReplyMarkup::ReplyInlineMarkup(r)) => {
                            r.rows.iter().all(|r| match r {
                                tl::enums::KeyboardButtonRow::Row(r) => r.buttons.iter().all(|b| {
                                    match b {
                                    tl::enums::KeyboardButton::InputKeyboardButtonUrlAuth(b) => {
                                        self.has_user(&b.bot)
                                    }
                                    tl::enums::KeyboardButton::InputKeyboardButtonUserProfile(
                                        b,
                                    ) => self.has_user(&b.user_id),
                                    tl::enums::KeyboardButton::UserProfile(b) => {
                                        self.has(b.user_id)
                                    }
                                    _ => true,
                                }
                                }),
                            })
                        }
                        None => true,
                    }
                    && match &m.entities {
                        Some(f) => f.iter().all(|e| match e {
                            tl::enums::MessageEntity::MentionName(m) => self.has(m.user_id),
                            tl::enums::MessageEntity::InputMessageEntityMentionName(m) => {
                                self.has_user(&m.user_id)
                            }
                            _ => true,
                        }),
                        None => true,
                    }
                    && match &m.replies {
                        Some(tl::enums::MessageReplies::Replies(r)) => match &r.recent_repliers {
                            Some(p) => p.iter().all(|p| self.has_peer(p)),
                            None => true,
                        },
                        None => true,
                    }
                    && match &m.reactions {
                        Some(tl::enums::MessageReactions::Reactions(r)) => {
                            match &r.recent_reactions {
                                Some(p) => p.iter().all(|r| match r {
                                    tl::enums::MessagePeerReaction::Reaction(x) => {
                                        self.has_peer(&x.peer_id)
                                    }
                                }),
                                None => true,
                            }
                        }
                        None => true,
                    }
            }
            tl::enums::Message::Service(m) => {
                (match &m.from_id {
                    Some(p) => self.has_peer(p),
                    None => true,
                }) && self.has_peer(&m.peer_id)
                    && match &m.reply_to {
                        Some(MRH::Header(r)) => match &r.reply_to_peer_id {
                            Some(p) => self.has_peer(p),
                            None => true,
                        },
                        Some(MRH::MessageReplyStoryHeader(r)) => self.has(r.user_id),
                        None => true,
                    }
                    && match &m.action {
                        MA::Empty => true,
                        MA::ChatCreate(_) => true,
                        MA::ChatEditTitle(_) => true,
                        MA::ChatEditPhoto(_) => true,
                        MA::ChatDeletePhoto => true,
                        MA::ChatAddUser(c) => c.users.iter().all(|u| self.has(*u)),
                        MA::ChatDeleteUser(c) => self.has(c.user_id),
                        MA::ChatJoinedByLink(c) => self.has(c.inviter_id),
                        MA::ChannelCreate(_) => true,
                        MA::ChatMigrateTo(c) => self.has(c.channel_id),
                        MA::ChannelMigrateFrom(_) => true,
                        MA::PinMessage => true,
                        MA::HistoryClear => true,
                        MA::GameScore(_) => true,
                        MA::PaymentSentMe(_) => true,
                        MA::PaymentSent(_) => true,
                        MA::PhoneCall(_) => true,
                        MA::ScreenshotTaken => true,
                        MA::CustomAction(_) => true,
                        MA::BotAllowed(_) => true,
                        MA::SecureValuesSentMe(_) => true,
                        MA::SecureValuesSent(_) => true,
                        MA::ContactSignUp => true,
                        MA::GeoProximityReached(c) => {
                            self.has_peer(&c.from_id) && self.has_peer(&c.to_id)
                        }
                        MA::GroupCall(_) => true,
                        MA::InviteToGroupCall(_) => true,
                        MA::SetMessagesTtl(_) => true,
                        MA::GroupCallScheduled(_) => true,
                        MA::SetChatTheme(_) => true,
                        MA::ChatJoinedByRequest => true,
                        MA::WebViewDataSentMe(_) => true,
                        MA::WebViewDataSent(_) => true,
                        MA::GiftPremium(_) => true,
                        MA::TopicCreate(_) => true,
                        MA::TopicEdit(_) => true,
                        MA::SuggestProfilePhoto(_) => true,
                        MA::RequestedPeer(c) => c.peers.iter().all(|p| self.has_peer(p)),
                        MA::SetChatWallPaper(_) => true,
                        MA::GiftCode(c) => match &c.boost_peer {
                            Some(p) => self.has_peer(p),
                            None => true,
                        },
                        MA::GiveawayLaunch => true,
                        MA::GiveawayResults(_) => true,
                    }
            }
        }
    }
}
