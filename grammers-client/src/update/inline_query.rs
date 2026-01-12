// Copyright 2020 - developers of the `grammers` project.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// https://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or https://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

use std::fmt;

use grammers_mtsender::InvocationError;
use grammers_session::types::{PeerId, PeerRef};
use grammers_session::updates::State;
use grammers_tl_types as tl;

use crate::message::InputMessage;
use crate::peer::{Peer, PeerMap, User};
use crate::{Client, utils};

/// Update that bots receive when a user performs an inline query such as `@bot query`.
#[derive(Clone)]
pub struct InlineQuery {
    pub raw: tl::enums::Update,
    pub state: State,
    pub(crate) client: Client,
    pub(crate) peers: PeerMap,
}

/// An inline query answer builder.
pub struct Answer {
    request: tl::functions::messages::SetInlineBotResults,
    client: Client,
}

impl InlineQuery {
    fn update(&self) -> &tl::types::UpdateBotInlineQuery {
        match &self.raw {
            tl::enums::Update::BotInlineQuery(update) => update,
            _ => unreachable!(),
        }
    }

    /// The [`Self::sender`]'s identifier.
    pub fn sender_id(&self) -> PeerId {
        PeerId::user(self.update().user_id)
    }

    /// Cached reference to the [`Self::sender`], if it is in cache.
    pub async fn sender_ref(&self) -> Option<PeerRef> {
        self.peers.get_ref(self.sender_id()).await
    }

    /// User that sent the query, if it is in cache.
    pub fn sender(&self) -> Option<&User> {
        match self.peers.get(self.sender_id()) {
            Some(Peer::User(user)) => Some(user),
            None => None,
            _ => unreachable!(),
        }
    }

    /// The text of the inline query.
    pub fn text(&self) -> &str {
        self.update().query.as_str()
    }

    /// The offset of the inline query.
    pub fn offset(&self) -> &str {
        self.update().offset.as_str()
    }

    /// Answer the inline query.
    // TODO: add example
    pub fn answer<T>(&self, results: impl IntoIterator<Item = T>) -> Answer
    where
        T: Into<tl::enums::InputBotInlineResult>,
    {
        Answer {
            request: tl::functions::messages::SetInlineBotResults {
                gallery: false,
                private: false,
                query_id: self.update().query_id,
                results: results.into_iter().map(Into::into).collect(),
                cache_time: 0,
                next_offset: None,
                switch_pm: None,
                switch_webview: None,
            },
            client: self.client.clone(),
        }
    }

    /// Type of the peer from which the inline query was sent.
    pub fn peer_type(&self) -> Option<tl::enums::InlineQueryPeerType> {
        self.update().peer_type.clone()
    }

    /// Query ID
    pub fn query_id(&self) -> i64 {
        self.update().query_id
    }
}

impl Answer {
    /// If set, the results will show as a gallery (grid).
    pub fn gallery(mut self) -> Self {
        self.request.gallery = true;
        self
    }

    /// If set, the results will be cached by the user's client (private) rather than by Telgram
    /// (not private).
    pub fn private(mut self) -> Self {
        self.request.private = true;
        self
    }

    /// For how long this result should be cached on the user's client. Defaults to 0 for no
    /// cache.
    pub fn cache_time(mut self, cache_time: i32) -> Self {
        self.request.cache_time = cache_time;
        self
    }

    /// The offset the client will send when the user scrolls the results and it repeats the
    /// request.
    pub fn next_offset(mut self, next_offset: impl Into<String>) -> Self {
        self.request.next_offset = Some(next_offset.into());
        self
    }

    /// If set, this text will be shown in the results to allow the user to switch to private
    /// messages.
    pub fn switch_pm(mut self, text: impl Into<String>, start_param: impl Into<String>) -> Self {
        self.request.switch_pm = Some(tl::enums::InlineBotSwitchPm::Pm(
            tl::types::InlineBotSwitchPm {
                text: text.into(),
                start_param: start_param.into(),
            },
        ));
        self
    }

    /// Answers the inline query with the given results.
    pub async fn send(self) -> Result<(), InvocationError> {
        self.client.invoke(&self.request).await?;
        Ok(())
    }
}

/// One of the possible answers to an [`InlineQuery`].
///
/// Article answers let you show an option that when clicked will cause the user to send a text message.
#[derive(Debug)]
pub struct Article {
    pub raw: tl::types::InputBotInlineResult,
}

impl Article {
    /// Creates an inline result with the given title.
    ///
    /// If selected by the user that made the inline query, the input message will be sent by them.
    pub fn new<S: Into<String>, M: Into<InputMessage>>(title: S, input_message: M) -> Self {
        let message = input_message.into();
        Self {
            raw: tl::types::InputBotInlineResult {
                id: utils::generate_random_id().to_string(),
                r#type: "article".into(),
                title: Some(title.into()),
                description: None,
                url: None,
                thumb: None,
                content: None,
                // TODO: also allow other types of messages than text
                send_message: tl::enums::InputBotInlineMessage::Text(
                    tl::types::InputBotInlineMessageText {
                        no_webpage: !message.link_preview,
                        invert_media: message.invert_media,
                        message: message.text,
                        entities: Some(message.entities),
                        reply_markup: message.reply_markup,
                    },
                ),
            },
        }
    }

    /// Unique identifier of the result.
    ///
    /// By default, a random string will be used.
    pub fn id(mut self, result_id: impl Into<String>) -> Self {
        self.raw.id = result_id.into();
        self
    }

    /// Short description of the result.
    pub fn description(mut self, description: impl Into<String>) -> Self {
        self.raw.description = Some(description.into());
        self
    }

    /// URL of the result.
    pub fn url(mut self, url: impl Into<String>) -> Self {
        self.raw.url = Some(url.into());
        self
    }

    /// URL of the thumbnail for the result.
    ///
    /// Must point to a suitable JPEG image.
    pub fn thumb_url(mut self, thumb_url: impl Into<String>) -> Self {
        self.raw.thumb = Some(tl::enums::InputWebDocument::Document(
            tl::types::InputWebDocument {
                url: thumb_url.into(),
                size: 0,
                mime_type: "image/jpeg".into(),
                attributes: vec![],
            },
        ));
        self
    }
}

impl From<Article> for tl::enums::InputBotInlineResult {
    fn from(article: Article) -> Self {
        tl::enums::InputBotInlineResult::Result(article.raw)
    }
}

impl fmt::Debug for InlineQuery {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("InlineQuery")
            .field("text", &self.text())
            .field("peer_type", &self.peer_type())
            .field("sender", &self.sender())
            .field("query_id", &self.query_id())
            .finish()
    }
}
