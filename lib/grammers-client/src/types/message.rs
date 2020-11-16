// Copyright 2020 - developers of the `grammers` project.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// https://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or https://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.
use crate::ext::MessageMediaExt;
use crate::{types, ClientHandle};
use chrono::{DateTime, NaiveDateTime, Utc};
use grammers_mtsender::InvocationError;
use grammers_tl_types as tl;
use std::io;
use std::path::Path;

/// Represents a Telegram message, which includes text messages, messages with media, and service
/// messages.
///
/// This message should be treated as a snapshot in time, that is, if the message is edited while
/// using this object, those changes won't alter this structure.
#[derive(Clone)]
pub struct Message {
    // Message services are a trimmed-down version of normal messages, but with `action`.
    //
    // Using `enum` just for that would clutter all methods with `match`, so instead service
    // messages are interpreted as messages and their action stored separatedly.
    pub(crate) msg: tl::types::Message,
    pub(crate) action: Option<tl::enums::MessageAction>,
    pub(crate) client: ClientHandle,
}

impl Message {
    pub(crate) fn new(client: &ClientHandle, message: tl::enums::Message) -> Option<Self> {
        match message {
            // Don't even bother to expose empty messages to the user, even if they have an ID.
            tl::enums::Message::Empty(_) => None,
            tl::enums::Message::Message(msg) => Some(Message {
                msg,
                action: None,
                client: client.clone(),
            }),
            tl::enums::Message::Service(msg) => Some(Message {
                msg: tl::types::Message {
                    out: msg.out,
                    mentioned: msg.mentioned,
                    media_unread: msg.media_unread,
                    silent: msg.silent,
                    post: msg.post,
                    from_scheduled: false,
                    legacy: msg.legacy,
                    edit_hide: false,
                    pinned: false,
                    id: msg.id,
                    from_id: msg.from_id,
                    peer_id: msg.peer_id,
                    fwd_from: None,
                    via_bot_id: None,
                    reply_to: msg.reply_to,
                    date: msg.date,
                    message: String::new(),
                    media: None,
                    reply_markup: None,
                    entities: None,
                    views: None,
                    forwards: None,
                    replies: None,
                    edit_date: None,
                    post_author: None,
                    grouped_id: None,
                    restriction_reason: None,
                },
                action: Some(msg.action),
                client: client.clone(),
            }),
        }
    }

    /// Whether the message is outgoing (i.e. you sent this message to some other chat) or
    /// incoming (i.e. someone else sent it to you or the chat).
    pub fn outgoing(&self) -> bool {
        self.msg.out
    }

    /// Whether you were mentioned in this message or not.
    ///
    /// This includes @username mentions, text mentions, and messages replying to one of your
    /// previous messages (even if it contains no mention in the message text).
    pub fn mentioned(&self) -> bool {
        self.msg.mentioned
    }

    /// Whether you have read the media in this message or not.
    ///
    /// Most commonly, these are voice notes that you have not played yet.
    pub fn media_unread(&self) -> bool {
        self.msg.media_unread
    }

    /// Whether the message should notify people with sound or not.
    pub fn silent(&self) -> bool {
        self.msg.silent
    }

    /// Whether this message is a post in a broadcast channel or not.
    pub fn post(&self) -> bool {
        self.msg.post
    }

    /// Whether this message was originated from a previously-scheduled message or not.
    pub fn from_scheduled(&self) -> bool {
        self.msg.from_scheduled
    }

    // `legacy` is not exposed, though it can be if it proves to be useful

    /// Whether the edited mark of this message is edited should be hidden (e.g. in GUI clients)
    /// or shown.
    pub fn edit_hide(&self) -> bool {
        self.msg.edit_hide
    }

    /// Whether this message is currently pinned or not.
    pub fn pinned(&self) -> bool {
        self.msg.pinned
    }

    /// The ID of this message.
    ///
    /// Message identifiers are counters that start at 1 and grow by 1 for each message produced.
    ///
    /// Every channel has its own unique message counter. This counter is the same for all users,
    /// but unique to each channel.
    ///
    /// Every account has another unique message counter which is used for private conversations
    /// and small group chats. This means different accounts will likely have different message
    /// identifiers for the same message in a private conversation or small group chat. This also
    /// implies that the message identifier alone is enough to uniquely identify the message,
    /// without the need to know the chat ID.
    pub fn id(&self) -> i32 {
        self.msg.id
    }

    /// The sender of this message, if any.
    pub fn sender(&self) -> Option<tl::enums::Peer> {
        // TODO return entity or custom peer type
        // should we return the entire entity even if we don't have information? probably yes
        self.msg.from_id.clone()
    }

    /// The chat where this message was sent to.
    ///
    /// This might be the user you're talking to for private conversations, or the group or
    /// channel where the message was sent.
    pub fn chat(&self) -> tl::enums::Peer {
        // TODO return entity or custom peer type
        self.msg.peer_id.clone()
    }

    /// If this message was forwarded from a previous message, return the header with information
    /// about that forward.
    pub fn forward_header(&self) -> Option<tl::enums::MessageFwdHeader> {
        self.msg.fwd_from.clone()
    }

    /// If this message was sent @via some inline bot, return the bot's user identifier.
    pub fn via_bot_id(&self) -> Option<i32> {
        self.msg.via_bot_id
    }

    /// If this message is replying to a previous message, return the header with information
    /// about that reply.
    pub fn reply_header(&self) -> Option<tl::enums::MessageReplyHeader> {
        self.msg.reply_to.clone()
    }

    /// The date when this message was produced.
    pub fn date(&self) -> DateTime<Utc> {
        DateTime::<Utc>::from_utc(NaiveDateTime::from_timestamp(self.msg.date as i64, 0), Utc)
    }

    /// The message's text.
    ///
    /// For service messages, this will be the empty strings.
    ///
    /// If the message has media, this text is the caption commonly displayed underneath it.
    pub fn text(&self) -> &str {
        &self.msg.message
    }

    // TODO `markdown_text`, `html_text`

    /// The media displayed by this message, if any.
    ///
    /// This not only includes photos or videos, but also contacts, polls, documents, locations
    /// and many other types.
    pub fn media(&self) -> Option<tl::enums::MessageMedia> {
        self.msg.media.clone()
    }

    /// If the message has a reply markup (which can happen for messages produced by bots),
    /// returns said markup.
    pub fn reply_markup(&self) -> Option<tl::enums::ReplyMarkup> {
        self.msg.reply_markup.clone()
    }

    /// The "entities" used to format this message, such as bold, italic, with their offsets and
    /// lengths.
    pub fn formatting_entities(&self) -> Option<&Vec<tl::enums::MessageEntity>> {
        // TODO correct the offsets and lengths to match the byte offsets
        self.msg.entities.as_ref()
    }

    /// How many views does this message have, when applicable.
    ///
    /// The same user account can contribute to increment this counter indefinitedly, however
    /// there is a server-side cooldown limitting how fast it can happen (several hours).
    pub fn view_count(&self) -> Option<i32> {
        self.msg.views
    }

    /// How many times has this message been forwarded, when applicable.
    pub fn forward_count(&self) -> Option<i32> {
        self.msg.forwards
    }

    /// How many replies does this message have, when applicable.
    pub fn reply_count(&self) -> Option<tl::enums::MessageReplies> {
        // TODO return int instead
        self.msg.replies.clone()
    }

    /// The date when this message was last edited.
    pub fn edit_date(&self) -> Option<DateTime<Utc>> {
        self.msg.edit_date.map(|date| {
            DateTime::<Utc>::from_utc(NaiveDateTime::from_timestamp(date as i64, 0), Utc)
        })
    }

    /// If this message was sent to a channel, return the name used by the author to post it.
    pub fn post_author(&self) -> Option<&str> {
        self.msg.post_author.as_ref().map(|author| author.as_ref())
    }

    /// If this message belongs to a group of messages, return the unique identifier for that
    /// group.
    ///
    /// This applies to albums of media, such as multiple photos grouped together.
    ///
    /// Note that there may be messages sent in between the messages forming a group.
    pub fn grouped_id(&self) -> Option<i64> {
        self.msg.grouped_id
    }

    /// A list of reasons on why this message is restricted.
    ///
    /// The message is not restricted if the return value is `None`.
    pub fn restriction_reason(&self) -> Option<&Vec<tl::enums::RestrictionReason>> {
        self.msg.restriction_reason.as_ref()
    }

    /// If this message is a service message, return the service action that occured.
    pub fn action(&self) -> Option<&tl::enums::MessageAction> {
        self.action.as_ref()
    }

    /// If this message is replying to another message, return the replied message ID.
    pub fn reply_to_message_id(&self) -> Option<i32> {
        if let Some(tl::enums::MessageReplyHeader::Header(m)) = &self.msg.reply_to {
            Some(m.reply_to_msg_id)
        } else {
            None
        }
    }

    /// Fetch the message that this message is replying to, or `None` if this message is not a
    /// reply to a previous message.
    ///
    /// Shorthand for `ClientHandle::get_reply_to_message`.
    pub async fn get_reply(&mut self) -> Result<Option<Self>, InvocationError> {
        self.client
            .clone() // TODO don't clone
            .get_reply_to_message(self.input_chat(), self)
            .await
    }

    /// Respond to this message by sending a new message in the same chat, but without directly
    /// replying to it.
    ///
    /// Shorthand for `ClientHandle::send_message`.
    pub async fn respond(&mut self, message: types::InputMessage) -> Result<(), InvocationError> {
        self.client.send_message(self.input_chat(), message).await
    }

    /// Directly reply to this message by sending a new message in the same chat that replies to
    /// it. This methods overrides the `reply_to` on the `InputMessage` to point to `self`.
    ///
    /// Shorthand for `ClientHandle::send_message`.
    pub async fn reply(&mut self, message: types::InputMessage) -> Result<(), InvocationError> {
        self.client
            .send_message(self.input_chat(), message.reply_to(Some(self.msg.id)))
            .await
    }

    /// Forward this message to another (or the same) chat.
    ///
    /// Shorthand for `ClientHandle::forward_messages`. If you need to forward multiple messages
    /// at once, consider using that method instead.
    pub async fn forward_to(
        &mut self,
        chat: &tl::enums::InputPeer,
    ) -> Result<Self, InvocationError> {
        // TODO return `Message`
        // When forwarding a single message, if it fails, Telegram should respond with RPC error.
        // If it succeeds we will have the single forwarded message present which we can unwrap.
        self.client
            .forward_messages(chat, &[self.msg.id], &self.input_chat())
            .await
            .map(|mut msgs| msgs.pop().unwrap().unwrap())
    }

    /// Edit this message to change its text or media.
    ///
    /// Shorthand for `ClientHandle::edit_message`.
    pub async fn edit(&mut self, new_message: types::InputMessage) -> Result<(), InvocationError> {
        self.client
            .edit_message(self.input_chat(), self.msg.id, new_message)
            .await
    }

    /// Delete this message for everyone.
    ///
    /// Shorthand for `ClientHandle::delete_messages`. If you need to delete multiple messages
    /// at once, consider using that method instead.
    pub async fn delete(&mut self) -> Result<(), InvocationError> {
        self.client
            .delete_messages(self.input_channel().as_ref(), &[self.msg.id])
            .await
            .map(drop)
    }

    /// Mark this message and all messages above it as read.
    ///
    /// Unlike `ClientHandle::mark_as_read`, this method only will mark the chat as read up to
    /// this message, not the entire chat.
    pub async fn mark_as_read(&mut self) -> Result<(), InvocationError> {
        if let Some(channel) = self.input_channel() {
            self.client
                .invoke(&tl::functions::channels::ReadHistory {
                    channel,
                    max_id: self.msg.id,
                })
                .await
                .map(drop)
        } else {
            self.client
                .invoke(&tl::functions::messages::ReadHistory {
                    peer: self.input_chat(),
                    max_id: self.msg.id,
                })
                .await
                .map(drop)
        }
    }

    pub(crate) fn input_chat(&self) -> tl::enums::InputPeer {
        todo!()
    }

    pub(crate) fn input_channel(&self) -> Option<tl::enums::InputChannel> {
        todo!()
    }

    /// Pin this message in the chat.
    ///
    /// Shorthand for `ClientHandle::pin_message`.
    pub async fn pin(&mut self) -> Result<(), InvocationError> {
        self.client
            .pin_message(&self.input_chat(), self.msg.id)
            .await
    }

    /// Unpin this message from the chat.
    ///
    /// Shorthand for `ClientHandle::unpin_message`.
    pub async fn unpin(&mut self) -> Result<(), InvocationError> {
        self.client
            .unpin_message(&self.input_chat(), self.msg.id)
            .await
    }

    /// Refetch this message, mutating all of its properties in-place.
    ///
    /// No changes will be made to the message if it fails to be fetched.
    ///
    /// Shorthand for `ClientHandle::get_messages_by_id`.
    pub async fn refetch(&mut self) -> Result<(), InvocationError> {
        // When fetching a single message, if it fails, Telegram should respond with RPC error.
        // If it succeeds we will have the single message present which we can unwrap.
        self.client
            .get_messages_by_id(self.input_channel().as_ref(), &[self.msg.id])
            .await?
            .pop()
            .unwrap()
            .unwrap();
        todo!("actually mutate self after get_messages_by_id returns `Message`")
    }

    /// Download the message media in this message if applicable.
    ///
    /// Returns `true` if there was media to download, or `false` otherwise.
    ///
    /// Shorthand for `ClientHandle::download_media`.
    pub async fn download_media<P: AsRef<Path>>(&mut self, path: P) -> Result<bool, io::Error> {
        if let Some(file) = self
            .msg
            .media
            .as_ref()
            .and_then(|media| media.to_input_file())
        {
            self.client.download_media(file, path).await.map(|_| true)
        } else {
            Ok(false)
        }
    }
}
