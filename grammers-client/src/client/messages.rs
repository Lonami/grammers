// Copyright 2020 - developers of the `grammers` project.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// https://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or https://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

//! Methods related to sending messages.
use crate::types::{InputReactions, IterBuffer, Message};
use crate::utils::{generate_random_id, generate_random_ids};
use crate::{Client, InputMedia, PeerMap, types};
use chrono::{DateTime, FixedOffset};
use grammers_mtsender::InvocationError;
use grammers_session::types::{PeerId, PeerKind, PeerRef};
use grammers_tl_types as tl;
use log::{Level, log_enabled, warn};
use std::collections::HashMap;
use tl::enums::InputPeer;

fn map_random_ids_to_messages(
    client: &Client,
    fetched_in: PeerRef,
    random_ids: &[i64],
    updates: tl::enums::Updates,
) -> Vec<Option<Message>> {
    match updates {
        tl::enums::Updates::Updates(tl::types::Updates {
            updates,
            users,
            chats,
            date: _,
            seq: _,
        }) => {
            let peers = PeerMap::new(users, chats);
            client.cache_peers_maybe(&peers);

            let rnd_to_id = updates
                .iter()
                .filter_map(|update| match update {
                    tl::enums::Update::MessageId(u) => Some((u.random_id, u.id)),
                    _ => None,
                })
                .collect::<HashMap<_, _>>();

            // TODO ideally this would use the same UpdateIter mechanism to make sure we don't
            //      accidentally miss variants
            let mut id_to_msg = updates
                .into_iter()
                .filter_map(|update| match update {
                    tl::enums::Update::NewMessage(tl::types::UpdateNewMessage {
                        message, ..
                    }) => Some(message),
                    tl::enums::Update::NewChannelMessage(tl::types::UpdateNewChannelMessage {
                        message,
                        ..
                    }) => Some(message),
                    tl::enums::Update::NewScheduledMessage(
                        tl::types::UpdateNewScheduledMessage { message, .. },
                    ) => Some(message),
                    _ => None,
                })
                .map(|message| Message::from_raw(client, message, Some(fetched_in.clone()), &peers))
                .map(|message| (message.id(), message))
                .collect::<HashMap<_, _>>();

            random_ids
                .iter()
                .map(|rnd| {
                    rnd_to_id
                        .get(rnd)
                        .and_then(|id| id_to_msg.remove(id))
                        .or_else(|| {
                            if id_to_msg.len() == 1 {
                                // If there's no random_id to map from, in the common case a single message
                                // should've been produced regardless, so try to recover by returning that.
                                id_to_msg.drain().next().map(|(_, m)| m)
                            } else {
                                None
                            }
                        })
                })
                .collect()
        }
        _ => panic!("API returned something other than Updates so messages can't be mapped"),
    }
}

pub(crate) fn parse_mention_entities(
    client: &Client,
    mut entities: Vec<tl::enums::MessageEntity>,
) -> Option<Vec<tl::enums::MessageEntity>> {
    if entities.is_empty() {
        return None;
    }

    if entities
        .iter()
        .any(|e| matches!(e, tl::enums::MessageEntity::MentionName(_)))
    {
        for entity in entities.iter_mut() {
            if let tl::enums::MessageEntity::MentionName(mention_name) = entity {
                *entity = tl::types::InputMessageEntityMentionName {
                    offset: mention_name.offset,
                    length: mention_name.length,
                    user_id: tl::enums::InputUser::User(tl::types::InputUser {
                        user_id: mention_name.user_id,
                        access_hash: client
                            .0
                            .session
                            .peer(PeerId::user(mention_name.user_id))
                            .map(|peer| peer.auth())
                            .unwrap_or_default()
                            .hash(),
                    }),
                }
                .into()
            }
        }
    }

    Some(entities)
}

const MAX_LIMIT: usize = 100;

impl<R: tl::RemoteCall<Return = tl::enums::messages::Messages>> IterBuffer<R, Message> {
    /// Fetches the total unless cached.
    ///
    /// The `request.limit` should be set to the right value before calling this method.
    async fn get_total(&mut self) -> Result<usize, InvocationError> {
        if let Some(total) = self.total {
            return Ok(total);
        }

        use tl::enums::messages::Messages;

        let total = match self.client.invoke(&self.request).await? {
            Messages::Messages(messages) => messages.messages.len(),
            Messages::Slice(messages) => messages.count as usize,
            Messages::ChannelMessages(messages) => messages.count as usize,
            Messages::NotModified(messages) => messages.count as usize,
        };
        self.total = Some(total);
        Ok(total)
    }

    /// Performs the network call, fills the buffer, and returns the `offset_rate` if any.
    ///
    /// The `request.limit` should be set to the right value before calling this method.
    async fn fill_buffer(
        &mut self,
        limit: i32,
        peer: Option<PeerRef>,
    ) -> Result<Option<i32>, InvocationError> {
        use tl::enums::messages::Messages;

        let (messages, users, chats, rate) = match self.client.invoke(&self.request).await? {
            Messages::Messages(m) => {
                self.last_chunk = true;
                self.total = Some(m.messages.len());
                (m.messages, m.users, m.chats, None)
            }
            Messages::Slice(m) => {
                // Can't rely on `count(messages) < limit` as the stop condition.
                // See https://github.com/LonamiWebs/Telethon/issues/3949 for more.
                //
                // If the highest fetched message ID is lower than or equal to the limit,
                // there can't be more messages after (highest ID - limit), because the
                // absolute lowest message ID is 1.
                self.last_chunk = m.messages.is_empty() || m.messages[0].id() <= limit;
                self.total = Some(m.count as usize);
                (m.messages, m.users, m.chats, m.next_rate)
            }
            Messages::ChannelMessages(m) => {
                self.last_chunk = m.messages.is_empty() || m.messages[0].id() <= limit;
                self.total = Some(m.count as usize);
                (m.messages, m.users, m.chats, None)
            }
            Messages::NotModified(_) => {
                panic!("API returned Messages::NotModified even though hash = 0")
            }
        };

        let peers = PeerMap::new(users, chats);
        self.client.cache_peers_maybe(&peers);

        let client = self.client.clone();
        self.buffer.extend(
            messages
                .into_iter()
                .map(|message| Message::from_raw(&client, message, peer, &peers)),
        );

        Ok(rate)
    }
}

pub type MessageIter = IterBuffer<tl::functions::messages::GetHistory, Message>;

impl MessageIter {
    fn new(client: &Client, peer: PeerRef) -> Self {
        Self::from_request(
            client,
            MAX_LIMIT,
            tl::functions::messages::GetHistory {
                peer: peer.into(),
                offset_id: 0,
                offset_date: 0,
                add_offset: 0,
                limit: 0,
                max_id: 0,
                min_id: 0,
                hash: 0,
            },
        )
    }

    pub fn offset_id(mut self, offset: i32) -> Self {
        self.request.offset_id = offset;
        self
    }

    pub fn max_date(mut self, offset: i32) -> Self {
        self.request.offset_date = offset;
        self
    }

    /// Determines how many messages there are in total.
    ///
    /// This only performs a network call if `next` has not been called before.
    pub async fn total(&mut self) -> Result<usize, InvocationError> {
        self.request.limit = 1;
        self.get_total().await
    }

    /// Return the next `Message` from the internal buffer, filling the buffer previously if it's
    /// empty.
    ///
    /// Returns `None` if the `limit` is reached or there are no messages left.
    pub async fn next(&mut self) -> Result<Option<Message>, InvocationError> {
        if let Some(result) = self.next_raw() {
            return result;
        }

        self.request.limit = self.determine_limit(MAX_LIMIT);
        self.fill_buffer(self.request.limit, Some(self.request.peer.clone().into()))
            .await?;

        // Don't bother updating offsets if this is the last time stuff has to be fetched.
        if !self.last_chunk && !self.buffer.is_empty() {
            let last = &self.buffer[self.buffer.len() - 1];
            self.request.offset_id = last.id();
            self.request.offset_date = last.date_timestamp();
        }

        Ok(self.pop_item())
    }
}

pub type SearchIter = IterBuffer<tl::functions::messages::Search, Message>;

impl SearchIter {
    fn new(client: &Client, peer: PeerRef) -> Self {
        // TODO let users tweak all the options from the request
        Self::from_request(
            client,
            MAX_LIMIT,
            tl::functions::messages::Search {
                peer: peer.into(),
                q: String::new(),
                from_id: None,
                saved_peer_id: None,
                saved_reaction: None,
                top_msg_id: None,
                filter: tl::enums::MessagesFilter::InputMessagesFilterEmpty,
                min_date: 0,
                max_date: 0,
                offset_id: 0,
                add_offset: 0,
                limit: 0,
                max_id: 0,
                min_id: 0,
                hash: 0,
            },
        )
    }

    pub fn offset_id(mut self, offset: i32) -> Self {
        self.request.offset_id = offset;
        self
    }

    /// Changes the query of the search. Telegram servers perform a somewhat fuzzy search over
    /// this query (so a word in singular may also return messages with the word in plural, for
    /// example).
    pub fn query(mut self, query: &str) -> Self {
        self.request.q = query.to_string();
        self
    }

    /// Restricts results to messages sent by the logged-in user
    pub fn sent_by_self(mut self) -> Self {
        self.request.from_id = Some(InputPeer::PeerSelf);
        self
    }

    /// Returns only messages with date bigger than date_time.
    ///
    /// ```
    /// use chrono::DateTime;
    ///
    /// # async fn f(peer: grammers_session::types::PeerRef, client: grammers_client::Client) -> Result<(), Box<dyn std::error::Error>> {
    /// // Search messages sent after Jan 1st, 2021
    /// let min_date = DateTime::parse_from_rfc3339("2021-01-01T00:00:00-00:00").unwrap();
    ///
    /// let mut messages = client.search_messages(peer).min_date(&min_date);
    ///
    /// # Ok(())
    /// # }
    /// ```
    pub fn min_date(mut self, date_time: &DateTime<FixedOffset>) -> Self {
        self.request.min_date = date_time.timestamp() as i32;
        self
    }

    /// Returns only messages with date smaller than date_time
    ///
    /// ```
    /// use chrono::DateTime;
    ///
    /// # async fn f(peer: grammers_session::types::PeerRef, client: grammers_client::Client) -> Result<(), Box<dyn std::error::Error>> {
    /// // Search messages sent before Dec, 25th 2022
    /// let max_date = DateTime::parse_from_rfc3339("2022-12-25T00:00:00-00:00").unwrap();
    ///
    /// let mut messages = client.search_messages(peer).max_date(&max_date);
    ///
    /// # Ok(())
    /// # }
    /// ```
    pub fn max_date(mut self, date_time: &DateTime<FixedOffset>) -> Self {
        self.request.max_date = date_time.timestamp() as i32;
        self
    }

    /// Changes the media filter. Only messages with this type of media will be fetched.
    pub fn filter(mut self, filter: tl::enums::MessagesFilter) -> Self {
        self.request.filter = filter;
        self
    }

    /// Determines how many messages there are in total.
    ///
    /// This only performs a network call if `next` has not been called before.
    pub async fn total(&mut self) -> Result<usize, InvocationError> {
        // Unlike most requests, a limit of 0 actually returns 0 and not a default amount
        // (as of layer 120).
        self.request.limit = 0;
        self.get_total().await
    }

    /// Return the next `Message` from the internal buffer, filling the buffer previously if it's
    /// empty.
    ///
    /// Returns `None` if the `limit` is reached or there are no messages left.
    pub async fn next(&mut self) -> Result<Option<Message>, InvocationError> {
        if let Some(result) = self.next_raw() {
            return result;
        }

        self.request.limit = self.determine_limit(MAX_LIMIT);
        self.fill_buffer(self.request.limit, Some(self.request.peer.clone().into()))
            .await?;

        // Don't bother updating offsets if this is the last time stuff has to be fetched.
        if !self.last_chunk && !self.buffer.is_empty() {
            let last = &self.buffer[self.buffer.len() - 1];
            self.request.offset_id = last.id();
            self.request.max_date = last.date_timestamp();
        }

        Ok(self.pop_item())
    }
}

pub type GlobalSearchIter = IterBuffer<tl::functions::messages::SearchGlobal, Message>;

impl GlobalSearchIter {
    fn new(client: &Client) -> Self {
        // TODO let users tweak all the options from the request
        Self::from_request(
            client,
            MAX_LIMIT,
            tl::functions::messages::SearchGlobal {
                folder_id: None,
                q: String::new(),
                filter: tl::enums::MessagesFilter::InputMessagesFilterEmpty,
                min_date: 0,
                max_date: 0,
                offset_rate: 0,
                offset_peer: tl::enums::InputPeer::Empty,
                offset_id: 0,
                limit: 0,
                broadcasts_only: false,
                groups_only: false,
                users_only: false,
            },
        )
    }

    pub fn offset_id(mut self, offset: i32) -> Self {
        self.request.offset_id = offset;
        self
    }

    /// Changes the query of the search. Telegram servers perform a somewhat fuzzy search over
    /// this query (so a word in singular may also return messages with the word in plural, for
    /// example).
    pub fn query(mut self, query: &str) -> Self {
        self.request.q = query.to_string();
        self
    }

    /// Changes the media filter. Only messages with this type of media will be fetched.
    pub fn filter(mut self, filter: tl::enums::MessagesFilter) -> Self {
        self.request.filter = filter;
        self
    }

    /// Determines how many messages there are in total.
    ///
    /// This only performs a network call if `next` has not been called before.
    pub async fn total(&mut self) -> Result<usize, InvocationError> {
        self.request.limit = 1;
        self.get_total().await
    }

    /// Return the next `Message` from the internal buffer, filling the buffer previously if it's
    /// empty.
    ///
    /// Returns `None` if the `limit` is reached or there are no messages left.
    pub async fn next(&mut self) -> Result<Option<Message>, InvocationError> {
        if let Some(result) = self.next_raw() {
            return result;
        }

        self.request.limit = self.determine_limit(MAX_LIMIT);
        let offset_rate = self.fill_buffer(self.request.limit, None).await?;

        // Don't bother updating offsets if this is the last time stuff has to be fetched.
        if !self.last_chunk && !self.buffer.is_empty() {
            let last = &self.buffer[self.buffer.len() - 1];
            self.request.offset_rate = offset_rate.unwrap_or(0);
            self.request.offset_peer = last.peer_ref().into();
            self.request.offset_id = last.id();
        }

        Ok(self.pop_item())
    }
}

/// Method implementations related to sending, modifying or getting messages.
impl Client {
    /// Sends a message to the desired peer.
    ///
    /// This method can also be used to send media such as photos, videos, documents, polls, etc.
    ///
    /// If you want to send a local file as media, you will need to use
    /// [`Client::upload_file`] first.
    ///
    /// Refer to [`InputMessage`] to learn more formatting options, such as using markdown or
    /// adding buttons under your message (if you're logged in as a bot).
    ///
    /// See also: [`Message::respond`], [`Message::reply`].
    ///
    /// # Examples
    ///
    /// ```
    /// # async fn f(peer: grammers_session::types::PeerRef, client: grammers_client::Client) -> Result<(), Box<dyn std::error::Error>> {
    /// client.send_message(peer, "Boring text message :-(").await?;
    ///
    /// use grammers_client::InputMessage;
    ///
    /// client.send_message(peer, InputMessage::new().text("Sneaky message").silent(true)).await?;
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// [`InputMessage`]: crate::InputMessage
    pub async fn send_message<C: Into<PeerRef>, M: Into<types::InputMessage>>(
        &self,
        peer: C,
        message: M,
    ) -> Result<Message, InvocationError> {
        let peer = peer.into();
        let message = message.into();
        let random_id = generate_random_id();
        let entities = parse_mention_entities(self, message.entities.clone());
        let updates = if let Some(media) = message.media.clone() {
            self.invoke(&tl::functions::messages::SendMedia {
                silent: message.silent,
                background: message.background,
                clear_draft: message.clear_draft,
                peer: peer.into(),
                reply_to: message.reply_to.map(|reply_to_msg_id| {
                    tl::types::InputReplyToMessage {
                        reply_to_msg_id,
                        top_msg_id: None,
                        reply_to_peer_id: None,
                        quote_text: None,
                        quote_entities: None,
                        quote_offset: None,
                        monoforum_peer_id: None,
                        todo_item_id: None,
                    }
                    .into()
                }),
                media,
                message: message.text.clone(),
                random_id,
                reply_markup: message.reply_markup.clone(),
                entities,
                schedule_date: message.schedule_date,
                schedule_repeat_period: None,
                send_as: None,
                noforwards: false,
                update_stickersets_order: false,
                invert_media: message.invert_media,
                quick_reply_shortcut: None,
                effect: None,
                allow_paid_floodskip: false,
                allow_paid_stars: None,
                suggested_post: None,
            })
            .await
        } else {
            self.invoke(&tl::functions::messages::SendMessage {
                no_webpage: !message.link_preview,
                silent: message.silent,
                background: message.background,
                clear_draft: message.clear_draft,
                peer: peer.into(),
                reply_to: message.reply_to.map(|reply_to_msg_id| {
                    tl::types::InputReplyToMessage {
                        reply_to_msg_id,
                        top_msg_id: None,
                        reply_to_peer_id: None,
                        quote_text: None,
                        quote_entities: None,
                        quote_offset: None,
                        monoforum_peer_id: None,
                        todo_item_id: None,
                    }
                    .into()
                }),
                message: message.text.clone(),
                random_id,
                reply_markup: message.reply_markup.clone(),
                entities,
                schedule_date: message.schedule_date,
                schedule_repeat_period: None,
                send_as: None,
                noforwards: false,
                update_stickersets_order: false,
                invert_media: message.invert_media,
                quick_reply_shortcut: None,
                effect: None,
                allow_paid_floodskip: false,
                allow_paid_stars: None,
                suggested_post: None,
            })
            .await
        }?;

        Ok(match updates {
            tl::enums::Updates::UpdateShortSentMessage(updates) => {
                let peer = if peer.id.kind() == PeerKind::UserSelf {
                    // from_raw_short_updates needs the peer ID
                    self.0.session.peer(peer.id).unwrap().into()
                } else {
                    peer
                };

                Message::from_raw_short_updates(self, updates, message, peer)
            }
            updates => {
                let updates_debug = if log_enabled!(Level::Warn) {
                    Some(updates.clone())
                } else {
                    None
                };

                match map_random_ids_to_messages(self, peer, &[random_id], updates)
                    .pop()
                    .flatten()
                {
                    Some(message) => message,
                    None => {
                        if let Some(updates) = updates_debug {
                            warn!(
                                "failed to find just-sent message in response updates; please report this:"
                            );
                            warn!("{:#?}", updates);
                        }
                        Message::from_raw(
                            self,
                            tl::enums::Message::Empty(tl::types::MessageEmpty {
                                id: 0,
                                peer_id: Some(peer.id.into()),
                            }),
                            Some(peer),
                            &PeerMap::empty(),
                        )
                    }
                }
            }
        })
    }

    /// Sends a album to the desired peer.
    ///
    /// This method can also be used to send a bunch of media such as photos, videos, documents, polls, etc.
    ///
    /// If you want to send a local file as media, you will need to use
    /// [`Client::upload_file`] first.
    ///
    /// Refer to [`InputMedia`] to learn more formatting options, such as using markdown.
    ///
    /// See also: [`Message::respond_album`], [`Message::reply_album`].
    ///
    /// # Examples
    ///
    /// ```
    /// # async fn f(peer: grammers_session::types::PeerRef, client: grammers_client::Client) -> Result<(), Box<dyn std::error::Error>> {
    /// use grammers_client::InputMedia;
    ///
    /// client.send_album(peer, vec![InputMedia::new().caption("A album").photo_url("https://example.com/cat.jpg")]).await?;
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// [`InputMedia`]: crate::InputMedia
    pub async fn send_album<C: Into<PeerRef>>(
        &self,
        peer: C,
        mut medias: Vec<InputMedia>,
    ) -> Result<Vec<Option<Message>>, InvocationError> {
        let peer = peer.into();
        let random_ids = generate_random_ids(medias.len());

        // Upload external files
        for media in medias.iter_mut() {
            let raw_media = media.media.clone().unwrap();

            if matches!(
                raw_media,
                tl::enums::InputMedia::UploadedPhoto(_)
                    | tl::enums::InputMedia::PhotoExternal(_)
                    | tl::enums::InputMedia::UploadedDocument(_)
                    | tl::enums::InputMedia::DocumentExternal(_)
            ) {
                let uploaded = self
                    .invoke(&tl::functions::messages::UploadMedia {
                        business_connection_id: None,
                        peer: peer.into(),
                        media: raw_media,
                    })
                    .await?;
                media.media = Some(
                    types::Media::from_raw(uploaded)
                        .unwrap()
                        .to_raw_input_media()
                        .unwrap(),
                );
            }
        }

        let first_media = medias.first().unwrap();

        let updates = self
            .invoke(&tl::functions::messages::SendMultiMedia {
                silent: false,
                background: false,
                clear_draft: false,
                peer: peer.into(),
                reply_to: first_media.reply_to.map(|reply_to_msg_id| {
                    tl::types::InputReplyToMessage {
                        reply_to_msg_id,
                        top_msg_id: None,
                        reply_to_peer_id: None,
                        quote_text: None,
                        quote_entities: None,
                        quote_offset: None,
                        monoforum_peer_id: None,
                        todo_item_id: None,
                    }
                    .into()
                }),
                schedule_date: None,
                multi_media: medias
                    .into_iter()
                    .zip(random_ids.iter())
                    .map(|(input_media, random_id)| {
                        let entities = parse_mention_entities(self, input_media.entities);
                        let raw_media = input_media.media.unwrap();

                        tl::enums::InputSingleMedia::Media(tl::types::InputSingleMedia {
                            media: raw_media,
                            random_id: *random_id,
                            message: input_media.caption,
                            entities,
                        })
                    })
                    .collect(),
                send_as: None,
                noforwards: false,
                update_stickersets_order: false,
                invert_media: false,
                quick_reply_shortcut: None,
                effect: None,
                allow_paid_floodskip: false,
                allow_paid_stars: None,
            })
            .await?;

        Ok(map_random_ids_to_messages(self, peer, &random_ids, updates))
    }

    /// Edits an existing message.
    ///
    /// Similar to [`Client::send_message`], advanced formatting can be achieved with the
    /// options offered by [`InputMessage`].
    ///
    /// See also: [`Message::edit`].
    ///
    /// # Examples
    ///
    /// ```
    /// # async fn f(peer: grammers_session::types::PeerRef, client: grammers_client::Client) -> Result<(), Box<dyn std::error::Error>> {
    /// let old_message_id = 123;
    /// client.edit_message(peer, old_message_id, "New text message").await?;
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// [`InputMessage`]: crate::InputMessage
    // TODO don't require nasty InputPeer
    pub async fn edit_message<C: Into<PeerRef>, M: Into<types::InputMessage>>(
        &self,
        peer: C,
        message_id: i32,
        new_message: M,
    ) -> Result<(), InvocationError> {
        let new_message = new_message.into();
        let entities = parse_mention_entities(self, new_message.entities);
        self.invoke(&tl::functions::messages::EditMessage {
            no_webpage: !new_message.link_preview,
            invert_media: new_message.invert_media,
            peer: peer.into().into(),
            id: message_id,
            message: Some(new_message.text),
            media: new_message.media,
            reply_markup: new_message.reply_markup,
            entities,
            schedule_date: new_message.schedule_date,
            schedule_repeat_period: None,
            quick_reply_shortcut_id: None,
        })
        .await?;

        Ok(())
    }

    /// Deletes up to 100 messages in a peer.
    ///
    /// <div class="stab unstable">
    ///
    /// **Warning**: when deleting messages from small group peers or private conversations, this
    /// method cannot validate that the provided message IDs actually belong to the input peer due
    /// to the way Telegram's API works. Make sure to pass correct [`Message::id`]'s.
    ///
    /// </div>
    ///
    /// The messages are deleted for both ends.
    ///
    /// The amount of deleted messages is returned (it might be less than the amount of input
    /// message IDs if some of them were already missing). It is not possible to find out which
    /// messages were actually deleted, but if the request succeeds, none of the specified message
    /// IDs will appear in the message history from that point on.
    ///
    /// See also: [`Message::delete`].
    ///
    /// # Examples
    ///
    /// ```
    /// # async fn f(peer: grammers_session::types::PeerRef, client: grammers_client::Client) -> Result<(), Box<dyn std::error::Error>> {
    /// let message_ids = [123, 456, 789];
    ///
    /// // Careful, these messages will be gone after the method succeeds!
    /// client.delete_messages(peer, &message_ids).await?;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn delete_messages<C: Into<PeerRef>>(
        &self,
        peer: C,
        message_ids: &[i32],
    ) -> Result<usize, InvocationError> {
        let peer = peer.into();
        let tl::enums::messages::AffectedMessages::Messages(affected) =
            if peer.id.kind() == PeerKind::Channel {
                self.invoke(&tl::functions::channels::DeleteMessages {
                    channel: peer.into(),
                    id: message_ids.to_vec(),
                })
                .await
            } else {
                self.invoke(&tl::functions::messages::DeleteMessages {
                    revoke: true,
                    id: message_ids.to_vec(),
                })
                .await
            }?;

        Ok(affected.pts_count as usize)
    }

    /// Forwards up to 100 messages from `source` into `destination`.
    ///
    /// For consistency with other methods, the peer upon which this request acts comes first
    /// (destination), and then the source peer.
    ///
    /// Returns the new forwarded messages in a list. Those messages that could not be forwarded
    /// will be `None`. The length of the resulting list is the same as the length of the input
    /// message IDs, and the indices from the list of IDs map to the indices in the result so
    /// you can find which messages were forwarded and which message they became.
    ///
    /// See also: [`Message::forward_to`].
    ///
    /// # Examples
    ///
    /// ```
    /// # async fn f(destination: grammers_session::types::PeerRef, source: grammers_session::types::PeerRef, client: grammers_client::Client) -> Result<(), Box<dyn std::error::Error>> {
    /// let message_ids = [123, 456, 789];
    ///
    /// let messages = client.forward_messages(destination, &message_ids, source).await?;
    /// let fwd_count = messages.into_iter().filter(Option::is_some).count();
    /// println!("Forwarded {} out of {} messages!", fwd_count, message_ids.len());
    /// # Ok(())
    /// # }
    /// ```
    pub async fn forward_messages<C: Into<PeerRef>, S: Into<PeerRef>>(
        &self,
        destination: C,
        message_ids: &[i32],
        source: S,
    ) -> Result<Vec<Option<Message>>, InvocationError> {
        // TODO let user customize more options
        let peer = destination.into();
        let request = tl::functions::messages::ForwardMessages {
            silent: false,
            background: false,
            with_my_score: false,
            drop_author: false,
            drop_media_captions: false,
            from_peer: source.into().into(),
            id: message_ids.to_vec(),
            random_id: generate_random_ids(message_ids.len()),
            to_peer: peer.into(),
            top_msg_id: None,
            reply_to: None,
            schedule_date: None,
            schedule_repeat_period: None,
            send_as: None,
            noforwards: false,
            quick_reply_shortcut: None,
            allow_paid_floodskip: false,
            effect: None,
            video_timestamp: None,
            allow_paid_stars: None,
            suggested_post: None,
        };
        let result = self.invoke(&request).await?;
        Ok(map_random_ids_to_messages(
            self,
            peer.into(),
            &request.random_id,
            result,
        ))
    }

    /// Gets the [`Message`] to which the input message is replying to.
    ///
    /// See also: [`Message::get_reply`].
    ///
    /// # Examples
    ///
    /// ```
    /// # async fn f(message: grammers_client::types::Message, client: grammers_client::Client) -> Result<(), Box<dyn std::error::Error>> {
    /// if let Some(reply) = client.get_reply_to_message(&message).await? {
    ///     println!("The reply said: {}", reply.text());
    /// }
    /// # Ok(())
    /// # }
    /// ```
    pub async fn get_reply_to_message(
        &self,
        message: &Message,
    ) -> Result<Option<Message>, InvocationError> {
        /// Helper method to fetch a single message by its input message.
        async fn get_message(
            client: &Client,
            peer: PeerRef,
            id: tl::enums::InputMessage,
        ) -> Result<(tl::enums::messages::Messages, bool), InvocationError> {
            if peer.id.kind() == PeerKind::Channel {
                client
                    .invoke(&tl::functions::channels::GetMessages {
                        id: vec![id],
                        channel: peer.into(),
                    })
                    .await
                    .map(|res| (res, false))
            } else {
                client
                    .invoke(&tl::functions::messages::GetMessages { id: vec![id] })
                    .await
                    .map(|res| (res, true))
            }
        }

        // TODO shouldn't this method take in a message id anyway?
        let peer = message.peer_ref();
        let reply_to_message_id = match message.reply_to_message_id() {
            Some(id) => id,
            None => return Ok(None),
        };

        let input_id =
            tl::enums::InputMessage::ReplyTo(tl::types::InputMessageReplyTo { id: message.id() });

        let (res, filter_req) = match get_message(self, peer, input_id).await {
            Ok(tup) => tup,
            Err(_) => {
                let input_id = tl::enums::InputMessage::Id(tl::types::InputMessageId {
                    id: reply_to_message_id,
                });
                get_message(self, peer, input_id).await?
            }
        };

        use tl::enums::messages::Messages;

        let (messages, users, chats) = match res {
            Messages::Messages(m) => (m.messages, m.users, m.chats),
            Messages::Slice(m) => (m.messages, m.users, m.chats),
            Messages::ChannelMessages(m) => (m.messages, m.users, m.chats),
            Messages::NotModified(_) => {
                panic!("API returned Messages::NotModified even though GetMessages was used")
            }
        };

        let peers = PeerMap::new(users, chats);
        self.cache_peers_maybe(&peers);
        Ok(messages
            .into_iter()
            .map(|m| Message::from_raw(self, m, Some(peer.into()), &peers))
            .next()
            .filter(|m| !filter_req || m.peer_ref().id == message.peer_ref().id))
    }

    /// Iterate over the message history of a peer, from most recent to oldest.
    ///
    /// # Examples
    ///
    /// ```
    /// # async fn f(peer: grammers_session::types::PeerRef, client: grammers_client::Client) -> Result<(), Box<dyn std::error::Error>> {
    /// // Note we're setting a reasonable limit, or we'd print out ALL the messages in peer!
    /// let mut messages = client.iter_messages(peer).limit(100);
    ///
    /// while let Some(message) = messages.next().await? {
    ///     println!("{}", message.text());
    /// }
    /// # Ok(())
    /// # }
    /// ```
    pub fn iter_messages<C: Into<PeerRef>>(&self, peer: C) -> MessageIter {
        MessageIter::new(self, peer.into())
    }

    /// Iterate over the messages that match certain search criteria.
    ///
    /// This allows you to search by text within a peer or filter by media among other things.
    ///
    /// # Examples
    ///
    /// ```
    /// # async fn f(peer: grammers_session::types::PeerRef, client: grammers_client::Client) -> Result<(), Box<dyn std::error::Error>> {
    /// // Let's print all the people who think grammers is cool.
    /// let mut messages = client.search_messages(peer).query("grammers is cool");
    ///
    /// while let Some(message) = messages.next().await? {
    ///     let sender = message.sender().unwrap();
    ///     println!("{}", sender.name().unwrap_or(&sender.id().to_string()));
    /// }
    /// # Ok(())
    /// # }
    /// ```
    pub fn search_messages<C: Into<PeerRef>>(&self, peer: C) -> SearchIter {
        SearchIter::new(self, peer.into())
    }

    /// Iterate over the messages that match certain search criteria, without being restricted to
    /// searching in a specific peer. The downside is that this global search supports less filters.
    ///
    /// This allows you to search by text within a peer or filter by media among other things.
    ///
    /// # Examples
    ///
    /// ```
    /// # async fn f(client: grammers_client::Client) -> Result<(), Box<dyn std::error::Error>> {
    /// // Let's print all the peers were people think grammers is cool.
    /// let mut messages = client.search_all_messages().query("grammers is cool");
    ///
    /// while let Some(message) = messages.next().await? {
    ///     println!("{}", message.peer().unwrap().name().unwrap_or(&message.peer().unwrap().id().to_string()));
    /// }
    /// # Ok(())
    /// # }
    /// ```
    pub fn search_all_messages(&self) -> GlobalSearchIter {
        GlobalSearchIter::new(self)
    }

    /// Get up to 100 messages using their ID.
    ///
    /// Returns the new retrieved messages in a list. Those messages that could not be retrieved
    /// or do not belong to the input peer will be `None`. The length of the resulting list is the
    /// same as the length of the input message IDs, and the indices from the list of IDs map to
    /// the indices in the result so you can map them into the new list.
    ///
    /// # Examples
    ///
    /// ```
    /// # async fn f(peer: grammers_session::types::PeerRef, client: grammers_client::Client) -> Result<(), Box<dyn std::error::Error>> {
    /// let message_ids = [123, 456, 789];
    ///
    /// let messages = client.get_messages_by_id(peer, &message_ids).await?;
    /// let count = messages.into_iter().filter(Option::is_some).count();
    /// println!("{} out of {} messages were deleted!", message_ids.len() - count, message_ids.len());
    /// # Ok(())
    /// # }
    /// ```
    pub async fn get_messages_by_id<C: Into<PeerRef>>(
        &self,
        peer: C,
        message_ids: &[i32],
    ) -> Result<Vec<Option<Message>>, InvocationError> {
        let peer = peer.into();
        let id = message_ids
            .iter()
            .map(|&id| tl::enums::InputMessage::Id(tl::types::InputMessageId { id }))
            .collect();

        let result = if peer.id.kind() == PeerKind::Channel {
            self.invoke(&tl::functions::channels::GetMessages {
                channel: peer.into(),
                id,
            })
            .await
        } else {
            self.invoke(&tl::functions::messages::GetMessages { id })
                .await
        }?;

        let (messages, users, chats) = match result {
            tl::enums::messages::Messages::Messages(m) => (m.messages, m.users, m.chats),
            tl::enums::messages::Messages::Slice(m) => (m.messages, m.users, m.chats),
            tl::enums::messages::Messages::ChannelMessages(m) => (m.messages, m.users, m.chats),
            tl::enums::messages::Messages::NotModified(_) => {
                panic!("API returned Messages::NotModified even though GetMessages was used")
            }
        };

        let peers = PeerMap::new(users, chats);
        self.cache_peers_maybe(&peers);
        let mut map = messages
            .into_iter()
            .map(|m| Message::from_raw(self, m, Some(peer.into()), &peers))
            .filter(|m| m.peer_ref().id == peer.id)
            .map(|m| (m.id(), m))
            .collect::<HashMap<_, _>>();

        Ok(message_ids.iter().map(|id| map.remove(id)).collect())
    }

    /// Get the latest pin from a peer.
    ///
    /// # Examples
    ///
    /// ```
    /// # async fn f(peer: grammers_session::types::PeerRef, client: grammers_client::Client) -> Result<(), Box<dyn std::error::Error>> {
    /// if let Some(message) = client.get_pinned_message(peer).await? {
    ///     println!("There is a message pinned: {}", message.text());
    /// } else {
    ///     println!("There are no messages pinned");
    /// }
    /// # Ok(())
    /// # }
    /// ```
    pub async fn get_pinned_message<C: Into<PeerRef>>(
        &self,
        peer: C,
    ) -> Result<Option<Message>, InvocationError> {
        let peer = peer.into();
        // TODO return types::Message and print its text in the example
        let id = vec![tl::enums::InputMessage::Pinned];

        let result = if peer.id.kind() == PeerKind::Channel {
            self.invoke(&tl::functions::channels::GetMessages {
                channel: peer.into(),
                id,
            })
            .await
        } else {
            self.invoke(&tl::functions::messages::GetMessages { id })
                .await
        }?;

        let (messages, users, chats) = match result {
            tl::enums::messages::Messages::Messages(m) => (m.messages, m.users, m.chats),
            tl::enums::messages::Messages::Slice(m) => (m.messages, m.users, m.chats),
            tl::enums::messages::Messages::ChannelMessages(m) => (m.messages, m.users, m.chats),
            tl::enums::messages::Messages::NotModified(_) => {
                panic!("API returned Messages::NotModified even though GetMessages was used")
            }
        };

        let peers = PeerMap::new(users, chats);
        self.cache_peers_maybe(&peers);
        Ok(messages
            .into_iter()
            .map(|m| Message::from_raw(self, m, Some(peer.into()), &peers))
            .find(|m| m.peer_ref().id == peer.id))
    }

    /// Pin a message in the peer. This will not notify any users.
    ///
    /// # Examples
    ///
    /// ```
    /// # async fn f(peer: grammers_session::types::PeerRef, client: grammers_client::Client) -> Result<(), Box<dyn std::error::Error>> {
    /// let message_id = 123;
    /// client.pin_message(peer, message_id).await?;
    /// # Ok(())
    /// # }
    /// ```
    // TODO return produced Option<service message>
    pub async fn pin_message<C: Into<PeerRef>>(
        &self,
        peer: C,
        message_id: i32,
    ) -> Result<(), InvocationError> {
        self.update_pinned(peer.into(), message_id, true).await
    }

    /// Unpin a message from the peer.
    ///
    /// # Examples
    ///
    /// ```
    /// # async fn f(peer: grammers_session::types::PeerRef, client: grammers_client::Client) -> Result<(), Box<dyn std::error::Error>> {
    /// let message_id = 123;
    /// client.unpin_message(peer, message_id).await?;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn unpin_message<C: Into<PeerRef>>(
        &self,
        peer: C,
        message_id: i32,
    ) -> Result<(), InvocationError> {
        self.update_pinned(peer.into(), message_id, false).await
    }

    async fn update_pinned(
        &self,
        peer: PeerRef,
        id: i32,
        pin: bool,
    ) -> Result<(), InvocationError> {
        self.invoke(&tl::functions::messages::UpdatePinnedMessage {
            silent: true,
            unpin: !pin,
            pm_oneside: false,
            peer: peer.into(),
            id,
        })
        .await
        .map(drop)
    }

    /// Unpin all currently-pinned messages from the peer.
    ///
    /// # Examples
    ///
    /// ```
    /// # async fn f(peer: grammers_session::types::PeerRef, client: grammers_client::Client) -> Result<(), Box<dyn std::error::Error>> {
    /// client.unpin_all_messages(peer).await?;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn unpin_all_messages<C: Into<PeerRef>>(
        &self,
        peer: C,
    ) -> Result<(), InvocationError> {
        self.invoke(&tl::functions::messages::UnpinAllMessages {
            peer: peer.into().into(),
            top_msg_id: None,
            saved_peer_id: None,
        })
        .await?;
        Ok(())
    }

    /// Send reaction.
    ///
    /// # Examples
    ///
    /// Via emoticon
    ///
    /// ```
    /// # async fn f(peer: grammers_session::types::PeerRef, client: grammers_client::Client) -> Result<(), Box<dyn std::error::Error>> {
    /// let message_id = 123;
    ///
    /// client.send_reactions(peer, message_id, "👍").await?;
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// Make animation big & Add to recent
    ///
    /// ```
    /// # async fn f(peer: grammers_session::types::PeerRef, client: grammers_client::Client) -> Result<(), Box<dyn std::error::Error>> {
    /// use grammers_client::types::InputReactions;
    ///
    /// let message_id = 123;
    /// let reactions = InputReactions::emoticon("🤯").big().add_to_recent();
    ///
    /// client.send_reactions(peer, message_id, reactions).await?;
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// Remove reactions
    ///
    /// ```
    /// # async fn f(peer: grammers_session::types::PeerRef, client: grammers_client::Client) -> Result<(), Box<dyn std::error::Error>> {
    /// use grammers_client::types::InputReactions;
    ///
    /// let message_id = 123;
    ///
    /// client.send_reactions(peer, message_id, InputReactions::remove()).await?;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn send_reactions<C: Into<PeerRef>, R: Into<InputReactions>>(
        &self,
        peer: C,
        message_id: i32,
        reactions: R,
    ) -> Result<(), InvocationError> {
        let reactions = reactions.into();

        self.invoke(&tl::functions::messages::SendReaction {
            big: reactions.big,
            add_to_recent: reactions.add_to_recent,
            peer: peer.into().into(),
            msg_id: message_id,
            reaction: Some(reactions.reactions),
        })
        .await?;

        Ok(())
    }
}
