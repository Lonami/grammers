// Copyright 2020 - developers of the `grammers` project.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// https://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or https://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

//! Methods related to sending messages.
use crate::types::{Chat, IterBuffer, Message};
use crate::utils::{generate_random_id, generate_random_ids};
use crate::{types, ChatMap, Client};
pub use grammers_mtsender::{AuthorizationError, InvocationError};
use grammers_tl_types as tl;
use std::collections::HashMap;

fn map_random_ids_to_messages(
    client: &Client,
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
            let chats = ChatMap::new(users, chats);

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
                    _ => None,
                })
                .filter_map(|message| Message::new(client, message, &chats))
                .map(|message| (message.msg.id, message))
                .collect::<HashMap<_, _>>();

            random_ids
                .iter()
                .map(|rnd| rnd_to_id.get(rnd).and_then(|id| id_to_msg.remove(id)))
                .collect()
        }
        _ => panic!("API returned something other than Updates so messages can't be mapped"),
    }
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
    async fn fill_buffer(&mut self, limit: i32) -> Result<Option<i32>, InvocationError> {
        use tl::enums::messages::Messages;

        let (messages, users, chats, rate) = match self.client.invoke(&self.request).await? {
            Messages::Messages(m) => {
                self.last_chunk = true;
                self.total = Some(m.messages.len());
                (m.messages, m.users, m.chats, None)
            }
            Messages::Slice(m) => {
                self.last_chunk = m.messages.len() < limit as usize;
                self.total = Some(m.count as usize);
                (m.messages, m.users, m.chats, m.next_rate)
            }
            Messages::ChannelMessages(m) => {
                self.last_chunk = m.messages.len() < limit as usize;
                self.total = Some(m.count as usize);
                (m.messages, m.users, m.chats, None)
            }
            Messages::NotModified(_) => {
                panic!("API returned Messages::NotModified even though hash = 0")
            }
        };

        let chats = ChatMap::new(users, chats);

        let client = self.client.clone();
        self.buffer.extend(
            messages
                .into_iter()
                .flat_map(|message| Message::new(&client, message, &chats)),
        );

        Ok(rate)
    }
}

pub type MessageIter = IterBuffer<tl::functions::messages::GetHistory, Message>;

impl MessageIter {
    fn new(client: &Client, peer: &Chat) -> Self {
        // TODO let users tweak all the options from the request
        Self::from_request(
            client,
            MAX_LIMIT,
            tl::functions::messages::GetHistory {
                peer: peer.to_input_peer(),
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
        self.fill_buffer(self.request.limit).await?;

        // Don't bother updating offsets if this is the last time stuff has to be fetched.
        if !self.last_chunk && !self.buffer.is_empty() {
            let last = &self.buffer[self.buffer.len() - 1];
            self.request.offset_id = last.msg.id;
            self.request.offset_date = last.msg.date;
        }

        Ok(self.pop_item())
    }
}

pub type SearchIter = IterBuffer<tl::functions::messages::Search, Message>;

impl SearchIter {
    fn new(client: &Client, peer: &Chat) -> Self {
        // TODO let users tweak all the options from the request
        Self::from_request(
            client,
            MAX_LIMIT,
            tl::functions::messages::Search {
                peer: peer.to_input_peer(),
                q: String::new(),
                from_id: None,
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

    /// Changes the query of the search. Telegram servers perform a somewhat fuzzy search over
    /// this query (so a world in singular may also return messages with the word in plural, for
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
        self.fill_buffer(self.request.limit).await?;

        // Don't bother updating offsets if this is the last time stuff has to be fetched.
        if !self.last_chunk && !self.buffer.is_empty() {
            let last = &self.buffer[self.buffer.len() - 1];
            self.request.offset_id = last.msg.id;
            self.request.max_date = last.msg.date;
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
            },
        )
    }

    /// Changes the query of the search. Telegram servers perform a somewhat fuzzy search over
    /// this query (so a world in singular may also return messages with the word in plural, for
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
        let offset_rate = self.fill_buffer(self.request.limit).await?;

        // Don't bother updating offsets if this is the last time stuff has to be fetched.
        if !self.last_chunk && !self.buffer.is_empty() {
            let last = &self.buffer[self.buffer.len() - 1];
            self.request.offset_rate = offset_rate.unwrap_or(0);
            self.request.offset_peer = last.chat().to_input_peer();
            self.request.offset_id = last.msg.id;
        }

        Ok(self.pop_item())
    }
}

/// Method implementations related to sending, modifying or getting messages.
impl Client {
    /// Sends a message to the desired chat.
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
    /// # async fn f(chat: grammers_client::types::Chat, mut client: grammers_client::Client) -> Result<(), Box<dyn std::error::Error>> {
    /// client.send_message(&chat, "Boring text message :-(".into()).await?;
    ///
    /// use grammers_client::InputMessage;
    ///
    /// client.send_message(&chat, InputMessage::text("Sneaky message").silent(true)).await?;
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// [`InputMessage`]: crate::InputMessage
    pub async fn send_message(
        &self,
        chat: &Chat,
        message: types::InputMessage,
    ) -> Result<Message, InvocationError> {
        let random_id = generate_random_id();
        let updates = if let Some(media) = message.media.clone() {
            self.invoke(&tl::functions::messages::SendMedia {
                silent: message.silent,
                background: message.background,
                clear_draft: message.clear_draft,
                peer: chat.to_input_peer(),
                reply_to_msg_id: message.reply_to,
                media,
                message: message.text.clone(),
                random_id,
                reply_markup: message.reply_markup.clone(),
                entities: if message.entities.is_empty() {
                    None
                } else {
                    Some(message.entities.clone())
                },
                schedule_date: message.schedule_date,
            })
            .await
        } else {
            self.invoke(&tl::functions::messages::SendMessage {
                no_webpage: !message.link_preview,
                silent: message.silent,
                background: message.background,
                clear_draft: message.clear_draft,
                peer: chat.to_input_peer(),
                reply_to_msg_id: message.reply_to,
                message: message.text.clone(),
                random_id,
                reply_markup: message.reply_markup.clone(),
                entities: if message.entities.is_empty() {
                    None
                } else {
                    Some(message.entities.clone())
                },
                schedule_date: message.schedule_date,
            })
            .await
        }?;

        Ok(match updates {
            tl::enums::Updates::UpdateShortSentMessage(updates) => {
                Message::from_short_updates(self, updates, message, chat)
            }
            updates => map_random_ids_to_messages(self, &[random_id], updates)
                .pop()
                .unwrap()
                .unwrap(),
        })
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
    /// # async fn f(chat: grammers_client::types::Chat, mut client: grammers_client::Client) -> Result<(), Box<dyn std::error::Error>> {
    /// let old_message_id = 123;
    /// client.edit_message(&chat, old_message_id, "New text message".into()).await?;
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// [`InputMessage`]: crate::InputMessage
    // TODO don't require nasty InputPeer
    pub async fn edit_message(
        &self,
        chat: &Chat,
        message_id: i32,
        new_message: types::InputMessage,
    ) -> Result<(), InvocationError> {
        self.invoke(&tl::functions::messages::EditMessage {
            no_webpage: !new_message.link_preview,
            peer: chat.to_input_peer(),
            id: message_id,
            message: Some(new_message.text),
            media: new_message.media,
            reply_markup: new_message.reply_markup,
            entities: Some(new_message.entities),
            schedule_date: new_message.schedule_date,
        })
        .await?;

        Ok(())
    }

    /// Deletes up to 100 messages in a chat.
    ///
    /// <div class="stab unstable">
    ///
    /// **Warning**: when deleting messages from small group chats or private conversations, this
    /// method cannot validate that the provided message IDs actually belong to the input chat due
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
    /// # async fn f(chat: grammers_client::types::Chat, mut client: grammers_client::Client) -> Result<(), Box<dyn std::error::Error>> {
    /// let message_ids = [123, 456, 789];
    ///
    /// // Careful, these messages will be gone after the method succeeds!
    /// client.delete_messages(&chat, &message_ids).await?;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn delete_messages(
        &self,
        chat: &Chat,
        message_ids: &[i32],
    ) -> Result<usize, InvocationError> {
        let tl::enums::messages::AffectedMessages::Messages(affected) =
            if let Some(channel) = chat.to_input_channel() {
                self.invoke(&tl::functions::channels::DeleteMessages {
                    channel,
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
    /// For consistency with other methods, the chat upon which this request acts comes first
    /// (destination), and then the source chat.
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
    /// # async fn f(destination: grammers_client::types::Chat, source: grammers_client::types::Chat, mut client: grammers_client::Client) -> Result<(), Box<dyn std::error::Error>> {
    /// let message_ids = [123, 456, 789];
    ///
    /// let messages = client.forward_messages(&destination, &message_ids, &source).await?;
    /// let fwd_count = messages.into_iter().filter(Option::is_some).count();
    /// println!("Forwarded {} out of {} messages!", fwd_count, message_ids.len());
    /// # Ok(())
    /// # }
    /// ```
    pub async fn forward_messages(
        &self,
        destination: &Chat,
        message_ids: &[i32],
        source: &Chat,
    ) -> Result<Vec<Option<Message>>, InvocationError> {
        // TODO let user customize more options
        let request = tl::functions::messages::ForwardMessages {
            silent: false,
            background: false,
            with_my_score: false,
            from_peer: source.to_input_peer(),
            id: message_ids.to_vec(),
            random_id: generate_random_ids(message_ids.len()),
            to_peer: destination.to_input_peer(),
            schedule_date: None,
        };
        let result = self.invoke(&request).await?;
        Ok(map_random_ids_to_messages(self, &request.random_id, result))
    }

    /// Gets the [`Message`] to which the input message is replying to.
    ///
    /// See also: [`Message::get_reply`].
    ///
    /// # Examples
    ///
    /// ```
    /// # async fn f(message: grammers_client::types::Message, mut client: grammers_client::Client) -> Result<(), Box<dyn std::error::Error>> {
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
            chat: &Chat,
            id: tl::enums::InputMessage,
        ) -> Result<(tl::enums::messages::Messages, bool), InvocationError> {
            if let Some(channel) = chat.to_input_channel() {
                client
                    .invoke(&tl::functions::channels::GetMessages {
                        id: vec![id],
                        channel,
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
        let chat = message.chat();
        let reply_to_message_id = match message.reply_to_message_id() {
            Some(id) => id,
            None => return Ok(None),
        };

        let input_id =
            tl::enums::InputMessage::ReplyTo(tl::types::InputMessageReplyTo { id: message.msg.id });

        let (res, filter_req) = match get_message(self, &chat, input_id).await {
            Ok(tup) => tup,
            Err(_) => {
                let input_id = tl::enums::InputMessage::Id(tl::types::InputMessageId {
                    id: reply_to_message_id,
                });
                get_message(self, &chat, input_id).await?
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

        let chats = ChatMap::new(users, chats);
        Ok(messages
            .into_iter()
            .flat_map(|m| Message::new(self, m, &chats))
            .next()
            .filter(|m| !filter_req || m.msg.peer_id == message.msg.peer_id))
    }

    /// Iterate over the message history of a chat, from most recent to oldest.
    ///
    /// # Examples
    ///
    /// ```
    /// # async fn f(chat: grammers_client::types::Chat, mut client: grammers_client::Client) -> Result<(), Box<dyn std::error::Error>> {
    /// // Note we're setting a reasonable limit, or we'd print out ALL the messages in chat!
    /// let mut messages = client.iter_messages(&chat).limit(100);
    ///
    /// while let Some(message) = messages.next().await? {
    ///     println!("{}", message.text());
    /// }
    /// # Ok(())
    /// # }
    /// ```
    pub fn iter_messages(&self, chat: &Chat) -> MessageIter {
        MessageIter::new(self, chat)
    }

    /// Iterate over the messages that match certain search criteria.
    ///
    /// This allows you to search by text within a chat or filter by media among other things.
    ///
    /// # Examples
    ///
    /// ```
    /// # async fn f(chat: grammers_client::types::Chat, mut client: grammers_client::Client) -> Result<(), Box<dyn std::error::Error>> {
    /// // Let's print all the people who think grammers is cool.
    /// let mut messages = client.search_messages(&chat).query("grammers is cool");
    ///
    /// while let Some(message) = messages.next().await? {
    ///     println!("{}", message.sender().unwrap().name());
    /// }
    /// # Ok(())
    /// # }
    /// ```
    pub fn search_messages(&self, chat: &Chat) -> SearchIter {
        SearchIter::new(self, chat)
    }

    /// Iterate over the messages that match certain search criteria, without being restricted to
    /// searching in a specific chat. The downside is that this global search supports less filters.
    ///
    /// This allows you to search by text within a chat or filter by media among other things.
    ///
    /// # Examples
    ///
    /// ```
    /// # async fn f(mut client: grammers_client::Client) -> Result<(), Box<dyn std::error::Error>> {
    /// // Let's print all the chats were people think grammers is cool.
    /// let mut messages = client.search_all_messages().query("grammers is cool");
    ///
    /// while let Some(message) = messages.next().await? {
    ///     println!("{}", message.chat().name());
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
    /// or do not belong to the input chat will be `None`. The length of the resulting list is the
    /// same as the length of the input message IDs, and the indices from the list of IDs map to
    /// the indices in the result so you can map them into the new list.
    ///
    /// # Examples
    ///
    /// ```
    /// # async fn f(chat: grammers_client::types::Chat, mut client: grammers_client::Client) -> Result<(), Box<dyn std::error::Error>> {
    /// let message_ids = [123, 456, 789];
    ///
    /// let messages = client.get_messages_by_id(&chat, &message_ids).await?;
    /// let count = messages.into_iter().filter(Option::is_some).count();
    /// println!("{} out of {} messages were deleted!", message_ids.len() - count, message_ids.len());
    /// # Ok(())
    /// # }
    /// ```
    pub async fn get_messages_by_id(
        &self,
        chat: &Chat,
        message_ids: &[i32],
    ) -> Result<Vec<Option<Message>>, InvocationError> {
        let id = message_ids
            .iter()
            .map(|&id| tl::enums::InputMessage::Id(tl::types::InputMessageId { id }))
            .collect();

        let result = if let Some(channel) = chat.to_input_channel() {
            self.invoke(&tl::functions::channels::GetMessages { channel, id })
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

        let chats = ChatMap::new(users, chats);
        let mut map = messages
            .into_iter()
            .flat_map(|m| Message::new(self, m, &chats))
            .filter(|m| m.chat().to_peer() == chat.to_peer())
            .map(|m| (m.msg.id, m))
            .collect::<HashMap<_, _>>();

        Ok(message_ids.iter().map(|id| map.remove(id)).collect())
    }

    /// Get the latest pin from a chat.
    ///
    /// # Examples
    ///
    /// ```
    /// # async fn f(chat: grammers_client::types::Chat, mut client: grammers_client::Client) -> Result<(), Box<dyn std::error::Error>> {
    /// if let Some(message) = client.get_pinned_message(&chat).await? {
    ///     println!("There is a message pinned in {}: {}", chat.name(), message.text());
    /// } else {
    ///     println!("There are no messages pinned in {}", chat.name());
    /// }
    /// # Ok(())
    /// # }
    /// ```
    pub async fn get_pinned_message(
        &self,
        chat: &Chat,
    ) -> Result<Option<Message>, InvocationError> {
        // TODO return types::Message and print its text in the example
        let id = vec![tl::enums::InputMessage::Pinned];

        let result = if let Some(channel) = chat.to_input_channel() {
            self.invoke(&tl::functions::channels::GetMessages { channel, id })
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

        let chats = ChatMap::new(users, chats);
        Ok(messages
            .into_iter()
            .flat_map(|m| Message::new(self, m, &chats))
            .find(|m| m.chat().to_peer() == chat.to_peer()))
    }

    /// Pin a message in the chat. This will not notify any users.
    ///
    /// # Examples
    ///
    /// ```
    /// # async fn f(chat: grammers_client::types::Chat, mut client: grammers_client::Client) -> Result<(), Box<dyn std::error::Error>> {
    /// let message_id = 123;
    /// client.pin_message(&chat, message_id).await?;
    /// # Ok(())
    /// # }
    /// ```
    // TODO return produced Option<service message>
    pub async fn pin_message(&self, chat: &Chat, message_id: i32) -> Result<(), InvocationError> {
        self.update_pinned(chat, message_id, true).await
    }

    /// Unpin a message from the chat.
    ///
    /// # Examples
    ///
    /// ```
    /// # async fn f(chat: grammers_client::types::Chat, mut client: grammers_client::Client) -> Result<(), Box<dyn std::error::Error>> {
    /// let message_id = 123;
    /// client.unpin_message(&chat, message_id).await?;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn unpin_message(&self, chat: &Chat, message_id: i32) -> Result<(), InvocationError> {
        self.update_pinned(chat, message_id, false).await
    }

    async fn update_pinned(&self, chat: &Chat, id: i32, pin: bool) -> Result<(), InvocationError> {
        self.invoke(&tl::functions::messages::UpdatePinnedMessage {
            silent: true,
            unpin: !pin,
            pm_oneside: false,
            peer: chat.to_input_peer(),
            id,
        })
        .await
        .map(drop)
    }

    /// Unpin all currently-pinned messages from the chat.
    ///
    /// # Examples
    ///
    /// ```
    /// # async fn f(chat: grammers_client::types::Chat, mut client: grammers_client::Client) -> Result<(), Box<dyn std::error::Error>> {
    /// client.unpin_all_messages(&chat).await?;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn unpin_all_messages(&self, chat: &Chat) -> Result<(), InvocationError> {
        self.invoke(&tl::functions::messages::UnpinAllMessages {
            peer: chat.to_input_peer(),
        })
        .await?;
        Ok(())
    }
}
