// Copyright 2020 - developers of the `grammers` project.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// https://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or https://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.
use crate::ChatMap;
#[cfg(any(feature = "markdown", feature = "html"))]
use crate::parsers;
use crate::types::reactions::InputReactions;
use crate::types::{InputMessage, Media, Photo};
use crate::{Client, types};
use crate::{InputMedia, utils};
use chrono::{DateTime, Utc};
use grammers_mtsender::InvocationError;
use grammers_session::PackedChat;
use grammers_tl_types as tl;
use std::fmt;
use std::sync::Arc;
use types::Chat;

#[cfg(feature = "fs")]
use std::{io, path::Path};

/// Represents a Telegram message, which includes text messages, messages with media, and service
/// messages.
///
/// This message should be treated as a snapshot in time, that is, if the message is edited while
/// using this object, those changes won't alter this structure.
#[derive(Clone)]
pub struct Message {
    pub raw: tl::enums::Message,
    pub(crate) fetched_in: Option<tl::enums::Peer>,
    pub(crate) client: Client,
    // When fetching messages or receiving updates, a set of chats will be present. A single
    // server response contains a lot of chats, and some might be related to deep layers of
    // a message action for instance. Keeping the entire set like this allows for cheaper clones
    // and moves, and saves us from worrying about picking out all the chats we care about.
    pub(crate) chats: Arc<ChatMap>,
}

impl Message {
    pub fn from_raw(
        client: &Client,
        message: tl::enums::Message,
        fetched_in: Option<tl::enums::Peer>,
        chats: &Arc<ChatMap>,
    ) -> Self {
        Self {
            raw: message,
            fetched_in,
            client: client.clone(),
            chats: Arc::clone(chats),
        }
    }

    pub fn from_raw_short_updates(
        client: &Client,
        updates: tl::types::UpdateShortSentMessage,
        input: InputMessage,
        chat: PackedChat,
    ) -> Self {
        Self {
            raw: tl::enums::Message::Message(tl::types::Message {
                out: updates.out,
                mentioned: false,
                media_unread: false,
                silent: input.silent,
                post: false, // TODO true if sent to broadcast channel
                from_scheduled: false,
                legacy: false,
                edit_hide: false,
                pinned: false,
                noforwards: false, // TODO true if channel has noforwads?
                video_processing_pending: false,
                invert_media: input.invert_media,
                id: updates.id,
                from_id: None, // TODO self
                from_boosts_applied: None,
                peer_id: chat.to_peer(),
                saved_peer_id: None,
                fwd_from: None,
                via_bot_id: None,
                reply_to: input.reply_to.map(|reply_to_msg_id| {
                    tl::types::MessageReplyHeader {
                        reply_to_scheduled: false,
                        forum_topic: false,
                        quote: false,
                        reply_to_msg_id: Some(reply_to_msg_id),
                        reply_to_peer_id: None,
                        reply_from: None,
                        reply_media: None,
                        reply_to_top_id: None,
                        quote_text: None,
                        quote_entities: None,
                        quote_offset: None,
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
                reactions: None,
                quick_reply_shortcut_id: None,
                via_business_bot_id: None,
                offline: false,
                effect: None,
                factcheck: None,
                report_delivery_until_date: None,
            }),
            fetched_in: None,
            client: client.clone(),
            chats: ChatMap::single(Chat::unpack(chat)),
        }
    }

    /// Whether the message is outgoing (i.e. you sent this message to some other chat) or
    /// incoming (i.e. someone else sent it to you or the chat).
    pub fn outgoing(&self) -> bool {
        match &self.raw {
            tl::enums::Message::Empty(_) => false,
            tl::enums::Message::Message(message) => message.out,
            tl::enums::Message::Service(message) => message.out,
        }
    }

    /// Whether you were mentioned in this message or not.
    ///
    /// This includes @username mentions, text mentions, and messages replying to one of your
    /// previous messages (even if it contains no mention in the message text).
    pub fn mentioned(&self) -> bool {
        match &self.raw {
            tl::enums::Message::Empty(_) => false,
            tl::enums::Message::Message(message) => message.mentioned,
            tl::enums::Message::Service(message) => message.mentioned,
        }
    }

    /// Whether you have read the media in this message or not.
    ///
    /// Most commonly, these are voice notes that you have not played yet.
    pub fn media_unread(&self) -> bool {
        match &self.raw {
            tl::enums::Message::Empty(_) => false,
            tl::enums::Message::Message(message) => message.media_unread,
            tl::enums::Message::Service(message) => message.media_unread,
        }
    }

    /// Whether the message should notify people with sound or not.
    pub fn silent(&self) -> bool {
        match &self.raw {
            tl::enums::Message::Empty(_) => false,
            tl::enums::Message::Message(message) => message.silent,
            tl::enums::Message::Service(message) => message.silent,
        }
    }

    /// Whether this message is a post in a broadcast channel or not.
    pub fn post(&self) -> bool {
        match &self.raw {
            tl::enums::Message::Empty(_) => false,
            tl::enums::Message::Message(message) => message.post,
            tl::enums::Message::Service(message) => message.post,
        }
    }

    /// Whether this message was originated from a previously-scheduled message or not.
    pub fn from_scheduled(&self) -> bool {
        match &self.raw {
            tl::enums::Message::Empty(_) => false,
            tl::enums::Message::Message(message) => message.from_scheduled,
            tl::enums::Message::Service(_) => false,
        }
    }

    // `legacy` is not exposed, though it can be if it proves to be useful

    /// Whether the edited mark of this message is edited should be hidden (e.g. in GUI clients)
    /// or shown.
    pub fn edit_hide(&self) -> bool {
        match &self.raw {
            tl::enums::Message::Empty(_) => false,
            tl::enums::Message::Message(message) => message.edit_hide,
            tl::enums::Message::Service(_) => false,
        }
    }

    /// Whether this message is currently pinned or not.
    pub fn pinned(&self) -> bool {
        match &self.raw {
            tl::enums::Message::Empty(_) => false,
            tl::enums::Message::Message(message) => message.pinned,
            tl::enums::Message::Service(_) => false,
        }
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
        self.raw.id()
    }

    pub(crate) fn peer_id(&self) -> &tl::enums::Peer {
        utils::peer_from_message(&self.raw)
            .or_else(|| self.fetched_in.as_ref())
            .expect("empty messages from updates should contain peer_id")
    }

    /// The sender of this message, if any.
    pub fn sender(&self) -> Option<types::Chat> {
        let from_id = match &self.raw {
            tl::enums::Message::Empty(_) => None,
            tl::enums::Message::Message(message) => message.from_id.as_ref(),
            tl::enums::Message::Service(message) => message.from_id.as_ref(),
        };
        from_id
            .or({
                // Incoming messages in private conversations don't include `from_id` since
                // layer 119, but the sender can only be the chat we're in.
                let peer_id = self.peer_id();
                if !self.outgoing() && matches!(peer_id, tl::enums::Peer::User(_)) {
                    Some(&peer_id)
                } else {
                    None
                }
            })
            .map(|from| utils::always_find_entity(from, &self.chats, &self.client))
    }

    /// The chat where this message was sent to.
    ///
    /// This might be the user you're talking to for private conversations, or the group or
    /// channel where the message was sent.
    pub fn chat(&self) -> types::Chat {
        utils::always_find_entity(self.peer_id(), &self.chats, &self.client)
    }

    /// If this message was forwarded from a previous message, return the header with information
    /// about that forward.
    pub fn forward_header(&self) -> Option<tl::enums::MessageFwdHeader> {
        match &self.raw {
            tl::enums::Message::Empty(_) => None,
            tl::enums::Message::Message(message) => message.fwd_from.clone(),
            tl::enums::Message::Service(_) => None,
        }
    }

    /// If this message was sent @via some inline bot, return the bot's user identifier.
    pub fn via_bot_id(&self) -> Option<i64> {
        match &self.raw {
            tl::enums::Message::Empty(_) => None,
            tl::enums::Message::Message(message) => message.via_bot_id,
            tl::enums::Message::Service(_) => None,
        }
    }

    /// If this message is replying to a previous message, return the header with information
    /// about that reply.
    pub fn reply_header(&self) -> Option<tl::enums::MessageReplyHeader> {
        match &self.raw {
            tl::enums::Message::Empty(_) => None,
            tl::enums::Message::Message(message) => message.reply_to.clone(),
            tl::enums::Message::Service(message) => message.reply_to.clone(),
        }
    }

    pub(crate) fn date_timestamp(&self) -> i32 {
        match &self.raw {
            tl::enums::Message::Empty(_) => 0,
            tl::enums::Message::Message(message) => message.date,
            tl::enums::Message::Service(message) => message.date,
        }
    }

    /// The date when this message was produced.
    pub fn date(&self) -> DateTime<Utc> {
        utils::date(self.date_timestamp())
    }

    /// The message's text.
    ///
    /// For service or empty messages, this will be the empty strings.
    ///
    /// If the message has media, this text is the caption commonly displayed underneath it.
    pub fn text(&self) -> &str {
        match &self.raw {
            tl::enums::Message::Empty(_) => "",
            tl::enums::Message::Message(message) => &message.message,
            tl::enums::Message::Service(_) => "",
        }
    }

    fn entities(&self) -> Option<&Vec<tl::enums::MessageEntity>> {
        match &self.raw {
            tl::enums::Message::Empty(_) => None,
            tl::enums::Message::Message(message) => message.entities.as_ref(),
            tl::enums::Message::Service(_) => None,
        }
    }

    /// Like [`text`](Self::text), but with the [`fmt_entities`](Self::fmt_entities)
    /// applied to produce a markdown string instead.
    ///
    /// Some formatting entities automatically added by Telegram, such as bot commands or
    /// clickable emails, are ignored in the generated string, as those do not need to be
    /// sent for Telegram to include them in the message.
    ///
    /// Formatting entities which cannot be represented in CommonMark without resorting to HTML,
    /// such as underline, are also ignored.
    #[cfg(feature = "markdown")]
    pub fn markdown_text(&self) -> String {
        if let Some(entities) = self.entities() {
            parsers::generate_markdown_message(self.text(), entities)
        } else {
            self.text().to_owned()
        }
    }

    /// Like [`text`](Self::text), but with the [`fmt_entities`](Self::fmt_entities)
    /// applied to produce a HTML string instead.
    ///
    /// Some formatting entities automatically added by Telegram, such as bot commands or
    /// clickable emails, are ignored in the generated string, as those do not need to be
    /// sent for Telegram to include them in the message.
    #[cfg(feature = "html")]
    pub fn html_text(&self) -> String {
        if let Some(entities) = self.entities() {
            parsers::generate_html_message(self.text(), entities)
        } else {
            self.text().to_owned()
        }
    }

    /// The media displayed by this message, if any.
    ///
    /// This not only includes photos or videos, but also contacts, polls, documents, locations
    /// and many other types.
    pub fn media(&self) -> Option<types::Media> {
        let media = match &self.raw {
            tl::enums::Message::Empty(_) => None,
            tl::enums::Message::Message(message) => message.media.clone(),
            tl::enums::Message::Service(_) => None,
        };
        media.and_then(Media::from_raw)
    }

    /// If the message has a reply markup (which can happen for messages produced by bots),
    /// returns said markup.
    pub fn reply_markup(&self) -> Option<tl::enums::ReplyMarkup> {
        match &self.raw {
            tl::enums::Message::Empty(_) => None,
            tl::enums::Message::Message(message) => message.reply_markup.clone(),
            tl::enums::Message::Service(_) => None,
        }
    }

    /// The formatting entities used to format this message, such as bold, italic, with their
    /// offsets and lengths.
    pub fn fmt_entities(&self) -> Option<&Vec<tl::enums::MessageEntity>> {
        // TODO correct the offsets and lengths to match the byte offsets
        self.entities()
    }

    /// How many views does this message have, when applicable.
    ///
    /// The same user account can contribute to increment this counter indefinitedly, however
    /// there is a server-side cooldown limitting how fast it can happen (several hours).
    pub fn view_count(&self) -> Option<i32> {
        match &self.raw {
            tl::enums::Message::Empty(_) => None,
            tl::enums::Message::Message(message) => message.views,
            tl::enums::Message::Service(_) => None,
        }
    }

    /// How many times has this message been forwarded, when applicable.
    pub fn forward_count(&self) -> Option<i32> {
        match &self.raw {
            tl::enums::Message::Empty(_) => None,
            tl::enums::Message::Message(message) => message.forwards,
            tl::enums::Message::Service(_) => None,
        }
    }

    /// How many replies does this message have, when applicable.
    pub fn reply_count(&self) -> Option<i32> {
        match &self.raw {
            tl::enums::Message::Message(tl::types::Message {
                replies: Some(tl::enums::MessageReplies::Replies(replies)),
                ..
            }) => Some(replies.replies),
            _ => None,
        }
    }

    /// React to this message.
    ///
    /// # Examples
    ///
    /// ```
    /// # async fn f(message: grammers_client::types::Message, client: grammers_client::Client) -> Result<(), Box<dyn std::error::Error>> {
    /// message.react("👍").await?;
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// Make animation big & Add to recent
    ///
    /// ```
    /// # async fn f(message: grammers_client::types::Message, client: grammers_client::Client) -> Result<(), Box<dyn std::error::Error>> {
    /// use grammers_client::types::InputReactions;
    ///
    /// let reactions = InputReactions::emoticon("🤯").big().add_to_recent();
    ///
    /// message.react(reactions).await?;
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// Remove reactions
    ///
    /// ```
    /// # async fn f(message: grammers_client::types::Message, client: grammers_client::Client) -> Result<(), Box<dyn std::error::Error>> {
    /// use grammers_client::types::InputReactions;
    ///
    /// message.react(InputReactions::remove()).await?;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn react<R: Into<InputReactions>>(
        &self,
        reactions: R,
    ) -> Result<(), InvocationError> {
        self.client
            .send_reactions(self.chat(), self.id(), reactions)
            .await?;
        Ok(())
    }

    /// How many reactions does this message have, when applicable.
    pub fn reaction_count(&self) -> Option<i32> {
        match &self.raw {
            tl::enums::Message::Message(tl::types::Message {
                reactions: Some(tl::enums::MessageReactions::Reactions(reactions)),
                ..
            }) => {
                let count = reactions
                    .results
                    .iter()
                    .map(|reaction: &tl::enums::ReactionCount| {
                        let tl::enums::ReactionCount::Count(reaction) = reaction;
                        reaction.count
                    })
                    .sum();
                Some(count)
            }
            _ => None,
        }
    }

    /// The date when this message was last edited.
    pub fn edit_date(&self) -> Option<DateTime<Utc>> {
        match &self.raw {
            tl::enums::Message::Empty(_) => None,
            tl::enums::Message::Message(message) => message.edit_date.map(utils::date),
            tl::enums::Message::Service(_) => None,
        }
    }

    /// If this message was sent to a channel, return the name used by the author to post it.
    pub fn post_author(&self) -> Option<&str> {
        match &self.raw {
            tl::enums::Message::Empty(_) => None,
            tl::enums::Message::Message(message) => message.post_author.as_deref(),
            tl::enums::Message::Service(_) => None,
        }
    }

    /// If this message belongs to a group of messages, return the unique identifier for that
    /// group.
    ///
    /// This applies to albums of media, such as multiple photos grouped together.
    ///
    /// Note that there may be messages sent in between the messages forming a group.
    pub fn grouped_id(&self) -> Option<i64> {
        match &self.raw {
            tl::enums::Message::Empty(_) => None,
            tl::enums::Message::Message(message) => message.grouped_id,
            tl::enums::Message::Service(_) => None,
        }
    }

    /// A list of reasons on why this message is restricted.
    ///
    /// The message is not restricted if the return value is `None`.
    pub fn restriction_reason(&self) -> Option<&Vec<tl::enums::RestrictionReason>> {
        match &self.raw {
            tl::enums::Message::Empty(_) => None,
            tl::enums::Message::Message(message) => message.restriction_reason.as_ref(),
            tl::enums::Message::Service(_) => None,
        }
    }

    /// If this message is a service message, return the service action that occured.
    pub fn action(&self) -> Option<&tl::enums::MessageAction> {
        match &self.raw {
            tl::enums::Message::Empty(_) => None,
            tl::enums::Message::Message(_) => None,
            tl::enums::Message::Service(message) => Some(&message.action),
        }
    }

    /// If this message is replying to another message, return the replied message ID.
    pub fn reply_to_message_id(&self) -> Option<i32> {
        match &self.raw {
            tl::enums::Message::Message(tl::types::Message {
                reply_to: Some(tl::enums::MessageReplyHeader::Header(header)),
                ..
            }) => header.reply_to_msg_id,
            _ => None,
        }
    }

    /// Fetch the message that this message is replying to, or `None` if this message is not a
    /// reply to a previous message.
    ///
    /// Shorthand for `Client::get_reply_to_message`.
    pub async fn get_reply(&self) -> Result<Option<Self>, InvocationError> {
        self.client
            .clone() // TODO don't clone
            .get_reply_to_message(self)
            .await
    }

    /// Respond to this message by sending a new message in the same chat, but without directly
    /// replying to it.
    ///
    /// Shorthand for `Client::send_message`.
    pub async fn respond<M: Into<InputMessage>>(
        &self,
        message: M,
    ) -> Result<Self, InvocationError> {
        self.client.send_message(&self.chat(), message).await
    }

    /// Respond to this message by sending a album in the same chat, but without directly
    /// replying to it.
    ///
    /// Shorthand for `Client::send_album`.
    pub async fn respond_album(
        &self,
        medias: Vec<InputMedia>,
    ) -> Result<Vec<Option<Self>>, InvocationError> {
        self.client.send_album(&self.chat(), medias).await
    }

    /// Directly reply to this message by sending a new message in the same chat that replies to
    /// it. This methods overrides the `reply_to` on the `InputMessage` to point to `self`.
    ///
    /// Shorthand for `Client::send_message`.
    pub async fn reply<M: Into<InputMessage>>(&self, message: M) -> Result<Self, InvocationError> {
        let message = message.into();
        self.client
            .send_message(&self.chat(), message.reply_to(Some(self.id())))
            .await
    }

    /// Directly reply to this message by sending a album in the same chat that replies to
    /// it. This methods overrides the `reply_to` on the first `InputMedia` to point to `self`.
    ///
    /// Shorthand for `Client::send_album`.
    pub async fn reply_album(
        &self,
        mut medias: Vec<InputMedia>,
    ) -> Result<Vec<Option<Self>>, InvocationError> {
        medias.first_mut().unwrap().reply_to = Some(self.id());
        self.client.send_album(&self.chat(), medias).await
    }

    /// Forward this message to another (or the same) chat.
    ///
    /// Shorthand for `Client::forward_messages`. If you need to forward multiple messages
    /// at once, consider using that method instead.
    pub async fn forward_to<C: Into<PackedChat>>(&self, chat: C) -> Result<Self, InvocationError> {
        // TODO return `Message`
        // When forwarding a single message, if it fails, Telegram should respond with RPC error.
        // If it succeeds we will have the single forwarded message present which we can unwrap.
        self.client
            .forward_messages(chat, &[self.id()], &self.chat())
            .await
            .map(|mut msgs| msgs.pop().unwrap().unwrap())
    }

    /// Edit this message to change its text or media.
    ///
    /// Shorthand for `Client::edit_message`.
    pub async fn edit<M: Into<InputMessage>>(&self, new_message: M) -> Result<(), InvocationError> {
        self.client
            .edit_message(&self.chat(), self.id(), new_message)
            .await
    }

    /// Delete this message for everyone.
    ///
    /// Shorthand for `Client::delete_messages`. If you need to delete multiple messages
    /// at once, consider using that method instead.
    pub async fn delete(&self) -> Result<(), InvocationError> {
        self.client
            .delete_messages(&self.chat(), &[self.id()])
            .await
            .map(drop)
    }

    /// Mark this message and all messages above it as read.
    ///
    /// Unlike `Client::mark_as_read`, this method only will mark the chat as read up to
    /// this message, not the entire chat.
    pub async fn mark_as_read(&self) -> Result<(), InvocationError> {
        let chat = self.chat().pack();
        if let Some(channel) = chat.try_to_input_channel() {
            self.client
                .invoke(&tl::functions::channels::ReadHistory {
                    channel,
                    max_id: self.id(),
                })
                .await
                .map(drop)
        } else {
            self.client
                .invoke(&tl::functions::messages::ReadHistory {
                    peer: chat.to_input_peer(),
                    max_id: self.id(),
                })
                .await
                .map(drop)
        }
    }

    /// Pin this message in the chat.
    ///
    /// Shorthand for `Client::pin_message`.
    pub async fn pin(&self) -> Result<(), InvocationError> {
        self.client.pin_message(&self.chat(), self.id()).await
    }

    /// Unpin this message from the chat.
    ///
    /// Shorthand for `Client::unpin_message`.
    pub async fn unpin(&self) -> Result<(), InvocationError> {
        self.client.unpin_message(&self.chat(), self.id()).await
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
            .get_messages_by_id(&self.chat(), &[self.id()])
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
    #[cfg(feature = "fs")]
    pub async fn download_media<P: AsRef<Path>>(&self, path: P) -> Result<bool, io::Error> {
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
