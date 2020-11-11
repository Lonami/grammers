// Copyright 2020 - developers of the `grammers` project.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// https://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or https://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

//! Methods related to sending messages.

use crate::types::IterBuffer;
use crate::{ext::MessageExt, types, ClientHandle, EntitySet};
pub use grammers_mtsender::{AuthorizationError, InvocationError};
use grammers_tl_types as tl;
use std::collections::HashMap;
use std::time::{SystemTime, UNIX_EPOCH};

/// Generate a random message ID suitable for `send_message`.
fn generate_random_message_id() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system time is before epoch")
        .as_nanos() as i64
}

// TODO these functions might cause issues if the system does not give enough time precision
fn generate_random_message_ids(n: usize) -> Vec<i64> {
    let start = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system time is before epoch")
        .as_nanos() as i64;

    (0..n as i64).map(|i| start + i).collect()
}

fn message_id(message: &tl::enums::Message) -> i32 {
    match message {
        tl::enums::Message::Empty(m) => m.id,
        tl::enums::Message::Message(m) => m.id,
        tl::enums::Message::Service(m) => m.id,
    }
}

fn message_date(message: &tl::enums::Message) -> Option<i32> {
    match message {
        tl::enums::Message::Empty(_) => None,
        tl::enums::Message::Message(m) => Some(m.date),
        tl::enums::Message::Service(m) => Some(m.date),
    }
}

fn map_random_ids_to_messages(
    random_ids: &[i64],
    updates: tl::enums::Updates,
) -> Vec<Option<tl::enums::Message>> {
    match updates {
        tl::enums::Updates::Updates(tl::types::Updates {
            updates,
            users: _,
            chats: _,
            date: _,
            seq: _,
        }) => {
            let rnd_to_id = updates
                .iter()
                .filter_map(|update| match update {
                    tl::enums::Update::MessageId(u) => Some((u.random_id, u.id)),
                    _ => None,
                })
                .collect::<HashMap<_, _>>();

            let mut id_to_msg = updates
                .into_iter()
                .filter_map(|update| match update {
                    tl::enums::Update::NewMessage(u) => Some((message_id(&u.message), u.message)),
                    _ => None,
                })
                .collect::<HashMap<_, _>>();

            random_ids
                .into_iter()
                .map(|rnd| rnd_to_id.get(rnd).and_then(|id| id_to_msg.remove(id)))
                .collect()
        }
        _ => panic!("API returned something other than Updates so messages can't be mapped"),
    }
}

const MAX_LIMIT: usize = 100;

pub type MessageIter = IterBuffer<tl::functions::messages::GetHistory, tl::enums::Message>;

impl MessageIter {
    fn new(client: &ClientHandle, peer: tl::enums::InputPeer) -> MessageIter {
        // TODO let users tweak all the options from the request
        Self::from_request(
            client,
            MAX_LIMIT,
            tl::functions::messages::GetHistory {
                peer,
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
        if let Some(total) = self.total {
            return Ok(total);
        }

        use tl::enums::messages::Messages;

        self.request.limit = 1;
        let total = match self.client.invoke(&self.request).await? {
            Messages::Messages(messages) => messages.messages.len(),
            Messages::Slice(messages) => messages.count as usize,
            Messages::ChannelMessages(messages) => messages.count as usize,
            Messages::NotModified(messages) => messages.count as usize,
        };
        self.total = Some(total);
        Ok(total)
    }

    /// Return the next `Message` from the internal buffer, filling the buffer previously if it's
    /// empty.
    ///
    /// Returns `None` if the `limit` is reached or there are no messages left.
    pub async fn next(&mut self) -> Result<Option<tl::enums::Message>, InvocationError> {
        if let Some(result) = self.next_raw() {
            return result;
        }

        use tl::enums::messages::Messages;

        self.request.limit = self.determine_limit(MAX_LIMIT);
        let (messages, users, chats) = match self.client.invoke(&self.request).await? {
            Messages::Messages(m) => {
                self.last_chunk = true;
                self.total = Some(m.messages.len());
                (m.messages, m.users, m.chats)
            }
            Messages::Slice(m) => {
                self.last_chunk = m.messages.len() < self.request.limit as usize;
                self.total = Some(m.count as usize);
                (m.messages, m.users, m.chats)
            }
            Messages::ChannelMessages(m) => {
                self.last_chunk = m.messages.len() < self.request.limit as usize;
                self.total = Some(m.count as usize);
                (m.messages, m.users, m.chats)
            }
            Messages::NotModified(_) => {
                panic!("API returned Messages::NotModified even though hash = 0")
            }
        };

        let _entities = EntitySet::new(users, chats);

        self.buffer.extend(
            messages
                .into_iter()
                .filter(|message| !matches!(message, tl::enums::Message::Empty(_))),
        );

        // Don't bother updating offsets if this is the last time stuff has to be fetched.
        if !self.last_chunk && !self.buffer.is_empty() {
            let last = &self.buffer[self.buffer.len() - 1];
            self.request.offset_id = message_id(last);
            self.request.offset_date = message_date(last).unwrap();
        }

        Ok(self.pop_item())
    }
}

pub type SearchIter = IterBuffer<tl::functions::messages::Search, tl::enums::Message>;

impl SearchIter {
    fn new(client: &ClientHandle, peer: tl::enums::InputPeer) -> SearchIter {
        // TODO let users tweak all the options from the request
        Self::from_request(
            client,
            MAX_LIMIT,
            tl::functions::messages::Search {
                peer,
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
        if let Some(total) = self.total {
            return Ok(total);
        }

        use tl::enums::messages::Messages;

        // Unlike most requests, a limit of 0 actually returns 0 and not a default amount
        // (as of layer 120).
        self.request.limit = 0;
        let total = match self.client.invoke(&self.request).await? {
            Messages::Messages(messages) => messages.messages.len(),
            Messages::Slice(messages) => messages.count as usize,
            Messages::ChannelMessages(messages) => messages.count as usize,
            Messages::NotModified(messages) => messages.count as usize,
        };
        self.total = Some(total);
        Ok(total)
    }

    /// Return the next `Message` from the internal buffer, filling the buffer previously if it's
    /// empty.
    ///
    /// Returns `None` if the `limit` is reached or there are no messages left.
    pub async fn next(&mut self) -> Result<Option<tl::enums::Message>, InvocationError> {
        if let Some(result) = self.next_raw() {
            return result;
        }

        use tl::enums::messages::Messages;

        self.request.limit = self.determine_limit(MAX_LIMIT);
        let (messages, users, chats) = match self.client.invoke(&self.request).await? {
            Messages::Messages(m) => {
                self.last_chunk = true;
                self.total = Some(m.messages.len());
                (m.messages, m.users, m.chats)
            }
            Messages::Slice(m) => {
                self.last_chunk = m.messages.len() < self.request.limit as usize;
                self.total = Some(m.count as usize);
                (m.messages, m.users, m.chats)
            }
            Messages::ChannelMessages(m) => {
                self.last_chunk = m.messages.len() < self.request.limit as usize;
                self.total = Some(m.count as usize);
                (m.messages, m.users, m.chats)
            }
            Messages::NotModified(_) => {
                panic!("API returned Messages::NotModified even though hash = 0")
            }
        };

        let _entities = EntitySet::new(users, chats);

        self.buffer.extend(
            messages
                .into_iter()
                .filter(|message| !matches!(message, tl::enums::Message::Empty(_))),
        );

        // Don't bother updating offsets if this is the last time stuff has to be fetched.
        if !self.last_chunk && !self.buffer.is_empty() {
            let last = &self.buffer[self.buffer.len() - 1];
            self.request.offset_id = message_id(last);
            self.request.max_date = message_date(last).unwrap();
        }

        Ok(self.pop_item())
    }
}

impl ClientHandle {
    /// Sends a text message to the desired chat.
    // TODO don't require nasty InputPeer
    pub async fn send_message(
        &mut self,
        chat: tl::enums::InputPeer,
        message: types::Message,
    ) -> Result<(), InvocationError> {
        self.invoke(&tl::functions::messages::SendMessage {
            no_webpage: !message.link_preview,
            silent: message.silent,
            background: message.background,
            clear_draft: message.clear_draft,
            peer: chat,
            reply_to_msg_id: message.reply_to,
            message: message.text,
            random_id: generate_random_message_id(),
            reply_markup: message.reply_markup,
            entities: if message.entities.is_empty() {
                None
            } else {
                Some(message.entities)
            },
            schedule_date: message.schedule_date,
        })
        .await?;
        Ok(())
    }

    /// Edits an existing text message
    // TODO don't require nasty InputPeer
    // TODO Media
    pub async fn edit_message(
        &mut self,
        chat: tl::enums::InputPeer,
        message_id: i32,
        new_message: types::Message,
    ) -> Result<(), InvocationError> {
        self.invoke(&tl::functions::messages::EditMessage {
            no_webpage: !new_message.link_preview,
            peer: chat,
            id: message_id,
            message: Some(new_message.text),
            media: None,
            reply_markup: new_message.reply_markup,
            entities: Some(new_message.entities),
            schedule_date: new_message.schedule_date,
        })
        .await?;

        Ok(())
    }

    /// Deletes up to 100 messages in a chat.
    ///
    /// The `chat` must only be specified when deleting messages from a broadcast channel or
    /// megagroup, not when deleting from small group chats or private conversations.
    ///
    /// The messages are deleted for both ends.
    ///
    /// The amount of deleted messages is returned (it might be less than the amount of input
    /// message IDs if some of them were already missing). It is not possible to find out which
    /// messages were actually deleted, but if the request succeeds, none of the specified message
    /// IDs will appear in the message history from that point on.
    pub async fn delete_messages(
        &mut self,
        chat: Option<&tl::enums::InputChannel>,
        message_ids: &[i32],
    ) -> Result<usize, InvocationError> {
        let tl::enums::messages::AffectedMessages::Messages(affected) = if let Some(chat) = chat {
            self.invoke(&tl::functions::channels::DeleteMessages {
                channel: chat.clone(),
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
    pub async fn forward_messages(
        &mut self,
        destination: &tl::enums::InputPeer,
        message_ids: &[i32],
        source: &tl::enums::InputPeer,
    ) -> Result<Vec<Option<tl::enums::Message>>, InvocationError> {
        // TODO let user customize more options
        let request = tl::functions::messages::ForwardMessages {
            silent: false,
            background: false,
            with_my_score: false,
            from_peer: source.clone(),
            id: message_ids.to_vec(),
            random_id: generate_random_message_ids(message_ids.len()),
            to_peer: destination.clone(),
            schedule_date: None,
        };
        let result = self.invoke(&request).await?;
        Ok(map_random_ids_to_messages(&request.random_id, result))
    }

    async fn a_reply_msg(
        &mut self,
        chat: &tl::enums::InputPeer,
        id: tl::enums::InputMessage,
    ) -> (Option<tl::enums::messages::Messages>, bool) {
        if let tl::enums::InputPeer::Channel(chan) = chat {
            (
                self.invoke(&tl::functions::channels::GetMessages {
                    id: vec![id],
                    channel: tl::enums::InputChannel::Channel(tl::types::InputChannel {
                        channel_id: chan.channel_id,
                        access_hash: chan.access_hash,
                    }),
                })
                .await
                .ok(),
                false,
            )
        } else {
            (
                self.invoke(&tl::functions::messages::GetMessages { id: vec![id] })
                    .await
                    .ok(),
                true,
            )
        }
    }

    /// Gets the reply to message of a message
    /// Throws NotFound error if there's no reply to message
    // TODO don't require nasty InputPeer
    pub async fn get_reply_to_message(
        &mut self,
        chat: tl::enums::InputPeer,
        message: &tl::types::Message,
    ) -> Option<tl::types::Message> {
        let input_id =
            tl::enums::InputMessage::ReplyTo(tl::types::InputMessageReplyTo { id: message.id });

        let (mut res, mut filter_req) = self.a_reply_msg(&chat, input_id).await;
        if res.is_none() {
            let input_id = tl::enums::InputMessage::Id(tl::types::InputMessageId {
                id: message.reply_to_message_id()?,
            });
            let r = self.a_reply_msg(&chat, input_id).await;
            res = r.0;
            filter_req = r.1;
        }

        let mut reply_msg_l = match res? {
            tl::enums::messages::Messages::Messages(m) => Some(m.messages),
            tl::enums::messages::Messages::Slice(m) => Some(m.messages),
            tl::enums::messages::Messages::ChannelMessages(m) => Some(m.messages),
            _ => None,
        }?;

        if filter_req {
            let chat = message.chat();
            return reply_msg_l
                .into_iter()
                .filter_map(|m| {
                    if let tl::enums::Message::Message(msg) = m {
                        Some(msg)
                    } else {
                        None
                    }
                })
                .filter(|m| m.chat() == chat)
                .next();
        } else {
            if let tl::enums::Message::Message(msg) = reply_msg_l.remove(0) {
                return Some(msg);
            } else {
                return None;
            }
        }
    }

    // TODO don't keep this, it should be implicit
    pub async fn input_peer_for_username(
        &mut self,
        username: &str,
    ) -> Result<tl::enums::InputPeer, InvocationError> {
        if username.eq_ignore_ascii_case("me") {
            Ok(tl::enums::InputPeer::PeerSelf)
        } else if let Some(user) = self.resolve_username(username).await? {
            Ok(tl::types::InputPeerUser {
                user_id: user.id,
                access_hash: user.access_hash.unwrap(), // TODO don't unwrap
            }
            .into())
        } else {
            // TODO same rationale as IntoInput<tl::enums::InputPeer> for tl::types::User
            todo!("user without username not handled")
        }
    }

    /// Iterate over the message history of a chat, from most recent to oldest.
    pub fn iter_messages(&self, chat: tl::enums::InputPeer) -> MessageIter {
        MessageIter::new(self, chat)
    }

    /// Iterate over the messages that match certain search criteria.
    ///
    /// This allows you to search by text within a chat or filter by media among other things.
    pub fn search_messages(&self, chat: tl::enums::InputPeer) -> SearchIter {
        SearchIter::new(self, chat)
    }
}
