// Copyright 2020 - developers of the `grammers` project.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// https://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or https://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.
use crate::types::{InputMessage, Media, Photo};
use crate::utils;
use crate::ChatMap;
use crate::{types, Client};
use grammers_mtsender::InvocationError;
use grammers_tl_types as tl;
use std::fmt;
use std::io;
use std::path::Path;
use std::sync::Arc;
use types::Chat;

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
    pub(crate) client: Client,
    // When fetching messages or receiving updates, a set of chats will be present. A single
    // server response contains a lot of chats, and some might be related to deep layers of
    // a message action for instance. Keeping the entire set like this allows for cheaper clones
    // and moves, and saves us from worrying about picking out all the chats we care about.
    pub(crate) chats: Arc<types::ChatMap>,
}

impl Message {
    pub(crate) fn new(
        client: &Client,
        message: tl::enums::Message,
        chats: &Arc<types::ChatMap>,
    ) -> Option<Self> {
        match message {
            // Don't even bother to expose empty messages to the user, even if they have an ID.
            tl::enums::Message::Empty(_) => None,
            tl::enums::Message::Message(msg) => Some(Message {
                msg,
                action: None,
                client: client.clone(),
                chats: Arc::clone(chats),
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
                    ttl_period: msg.ttl_period,
                },
                action: Some(msg.action),
                client: client.clone(),
                chats: Arc::clone(chats),
            }),
        }
    }

    pub(crate) fn from_short_updates(
        client: &Client,
        updates: tl::types::UpdateShortSentMessage,
        input: InputMessage,
        chat: &Chat,
    ) -> Self {
        Self {
            msg: tl::types::Message {
                out: updates.out,
                mentioned: false,
                media_unread: false,
                silent: input.silent,
                post: false, // TODO true if sent to broadcast channel
                from_scheduled: false,
                legacy: false,
                edit_hide: false,
                pinned: false,
                id: updates.id,
                from_id: None, // TODO self
                peer_id: chat.to_peer(),
                fwd_from: None,
                via_bot_id: None,
                reply_to: input.reply_to.map(|reply_to_msg_id| {
                    tl::types::MessageReplyHeader {
                        reply_to_msg_id,
                        reply_to_peer_id: None,
                        reply_to_top_id: None,
                    }
                    .into()
                }),
                date: updates.date,
                message: input.text,
                media: updates.media,
                reply_markup: input.reply_markup,
                entities: updates.entities,
                views: None,
                forwards: None,
                replies: None,
                edit_date: None,
                post_author: None,
                grouped_id: None,
                restriction_reason: None,
                ttl_period: updates.ttl_period,
            },
            action: None,
            client: client.clone(),
            chats: ChatMap::single(chat),
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
    ///
    /// **You cannot use the message ID of User A when running as User B**, unless this message
    /// belongs to a megagroup or broadcast channel. Beware of this when using methods like
    /// [`Client::delete_messages`], which **cannot** validate the chat where the message
    /// should be deleted for those cases.
    pub fn id(&self) -> i32 {
        self.msg.id
    }

    /// The sender of this message, if any.
    pub fn sender(&self) -> Option<types::Chat> {
        self.msg
            .from_id
            .as_ref()
            .or_else(|| {
                // Incoming messages in private conversations don't include `from_id` since
                // layer 119, but the sender can only be the chat we're in.
                if !self.msg.out && matches!(self.msg.peer_id, tl::enums::Peer::User(_)) {
                    Some(&self.msg.peer_id)
                } else {
                    None
                }
            })
            .and_then(|from| self.chats.get(from))
            .cloned()
    }

    /// The chat where this message was sent to.
    ///
    /// This might be the user you're talking to for private conversations, or the group or
    /// channel where the message was sent.
    pub fn chat(&self) -> types::Chat {
        self.chats.get(&self.msg.peer_id).cloned().unwrap()
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
    pub fn date(&self) -> utils::Date {
        utils::date(self.msg.date)
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
    pub fn media(&self) -> Option<types::Media> {
        self.msg
            .media
            .clone()
            .and_then(|x| Media::from_raw(x, self.client.clone()))
    }

    /// If the message has a reply markup (which can happen for messages produced by bots),
    /// returns said markup.
    pub fn reply_markup(&self) -> Option<tl::enums::ReplyMarkup> {
        self.msg.reply_markup.clone()
    }

    /// The formatting entities used to format this message, such as bold, italic, with their
    /// offsets and lengths.
    pub fn fmt_entities(&self) -> Option<&Vec<tl::enums::MessageEntity>> {
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
    pub fn edit_date(&self) -> Option<utils::Date> {
        self.msg.edit_date.map(utils::date)
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
    /// Shorthand for `Client::get_reply_to_message`.
    pub async fn get_reply(&mut self) -> Result<Option<Self>, InvocationError> {
        self.client
            .clone() // TODO don't clone
            .get_reply_to_message(self)
            .await
    }

    /// Respond to this message by sending a new message in the same chat, but without directly
    /// replying to it.
    ///
    /// Shorthand for `Client::send_message`.
    pub async fn respond(&self, message: types::InputMessage) -> Result<Self, InvocationError> {
        self.client.send_message(&self.chat(), message).await
    }

    /// Directly reply to this message by sending a new message in the same chat that replies to
    /// it. This methods overrides the `reply_to` on the `InputMessage` to point to `self`.
    ///
    /// Shorthand for `Client::send_message`.
    pub async fn reply(&self, message: types::InputMessage) -> Result<Self, InvocationError> {
        self.client
            .send_message(&self.chat(), message.reply_to(Some(self.msg.id)))
            .await
    }

    /// Forward this message to another (or the same) chat.
    ///
    /// Shorthand for `Client::forward_messages`. If you need to forward multiple messages
    /// at once, consider using that method instead.
    pub async fn forward_to(&self, chat: &Chat) -> Result<Self, InvocationError> {
        // TODO return `Message`
        // When forwarding a single message, if it fails, Telegram should respond with RPC error.
        // If it succeeds we will have the single forwarded message present which we can unwrap.
        self.client
            .forward_messages(chat, &[self.msg.id], &self.chat())
            .await
            .map(|mut msgs| msgs.pop().unwrap().unwrap())
    }

    /// Edit this message to change its text or media.
    ///
    /// Shorthand for `Client::edit_message`.
    pub async fn edit(&mut self, new_message: types::InputMessage) -> Result<(), InvocationError> {
        self.client
            .edit_message(&self.chat(), self.msg.id, new_message)
            .await
    }

    /// Delete this message for everyone.
    ///
    /// Shorthand for `Client::delete_messages`. If you need to delete multiple messages
    /// at once, consider using that method instead.
    pub async fn delete(&mut self) -> Result<(), InvocationError> {
        self.client
            .delete_messages(&self.chat(), &[self.msg.id])
            .await
            .map(drop)
    }

    /// Mark this message and all messages above it as read.
    ///
    /// Unlike `Client::mark_as_read`, this method only will mark the chat as read up to
    /// this message, not the entire chat.
    pub async fn mark_as_read(&mut self) -> Result<(), InvocationError> {
        if let Some(channel) = self.chat().to_input_channel() {
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
                    peer: self.chat().to_input_peer(),
                    max_id: self.msg.id,
                })
                .await
                .map(drop)
        }
    }

    /// Pin this message in the chat.
    ///
    /// Shorthand for `Client::pin_message`.
    pub async fn pin(&mut self) -> Result<(), InvocationError> {
        self.client.pin_message(&self.chat(), self.msg.id).await
    }

    /// Unpin this message from the chat.
    ///
    /// Shorthand for `Client::unpin_message`.
    pub async fn unpin(&mut self) -> Result<(), InvocationError> {
        self.client.unpin_message(&self.chat(), self.msg.id).await
    }

    /// Refetch this message, mutating all of its properties in-place.
    ///
    /// No changes will be made to the message if it fails to be fetched.
    ///
    /// Shorthand for `Client::get_messages_by_id`.
    pub async fn refetch(&self) -> Result<(), InvocationError> {
        // When fetching a single message, if it fails, Telegram should respond with RPC error.
        // If it succeeds we will have the single message present which we can unwrap.
        self.client
            .get_messages_by_id(&self.chat(), &[self.msg.id])
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
    /// Shorthand for `Client::download_media`.
    pub async fn download_media<P: AsRef<Path>>(&mut self, path: P) -> Result<bool, io::Error> {
        // TODO probably encode failed download in error
        if let Some(media) = self.media() {
            self.client.download_media(&media, path).await.map(|_| true)
        } else {
            Ok(false)
        }
    }

    /// Get photo attached to the message if any.
    pub fn photo(&self) -> Option<Photo> {
        if let Media::Photo(photo) = self.media()? {
            return Some(photo);
        }

        None
    }
}

impl fmt::Debug for Message {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Message")
            .field("id", &self.id())
            .field("outgoing", &self.outgoing())
            .field("date", &self.date())
            .field("text", &self.text())
            .field("chat", &self.chat())
            .field("sender", &self.sender())
            .field("reply_to_message_id", &self.reply_to_message_id())
            .field("via_bot_id", &self.via_bot_id())
            .field("media", &self.media())
            .field("mentioned", &self.mentioned())
            .field("media_unread", &self.media_unread())
            .field("silent", &self.silent())
            .field("post", &self.post())
            .field("from_scheduled", &self.from_scheduled())
            .field("edit_hide", &self.edit_hide())
            .field("pinned", &self.pinned())
            .field("forward_header", &self.forward_header())
            .field("reply_header", &self.reply_header())
            .field("reply_markup", &self.reply_markup())
            .field("fmt_entities", &self.fmt_entities())
            .field("view_count", &self.view_count())
            .field("forward_count", &self.forward_count())
            .field("reply_count", &self.reply_count())
            .field("edit_date", &self.edit_date())
            .field("post_author", &self.post_author())
            .field("grouped_id", &self.grouped_id())
            .field("restriction_reason", &self.restriction_reason())
            .field("action", &self.action())
            .finish()
    }
}
