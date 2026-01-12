// Copyright 2020 - developers of the `grammers` project.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// https://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or https://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

use std::convert::TryInto;
use std::fmt;
use std::time::Duration;

use grammers_mtsender::InvocationError;
use grammers_session::types::{PeerId, PeerRef};
use grammers_session::updates::State;
use grammers_tl_types as tl;

use crate::Client;
use crate::message::{InputMessage, Message};
use crate::peer::{Peer, PeerMap};

/// Update that bots receive when a user presses one of the bot's inline callback buttons.
///
/// You should always [`CallbackQuery::answer`] these queries, even if you have no data to display
/// to the user, because otherwise they will think the bot is non-responsive (the button spinner
/// will timeout).
#[derive(Clone)]
pub struct CallbackQuery {
    pub raw: tl::enums::Update,
    pub state: State,
    pub(crate) client: Client,
    pub(crate) peers: PeerMap,
}

/// A callback query answer builder.
///
/// It will be executed once `.await`-ed. Modifying it after polling it once will have no effect.
pub struct Answer<'a> {
    query: &'a CallbackQuery,
    request: tl::functions::messages::SetBotCallbackAnswer,
}

impl CallbackQuery {
    /// The [`Self::peer`]'s identifier.
    pub fn peer_id(&self) -> PeerId {
        match &self.raw {
            tl::enums::Update::BotCallbackQuery(update) => update.peer.clone().into(),
            tl::enums::Update::InlineBotCallbackQuery(update) => PeerId::user(update.user_id),
            _ => unreachable!(),
        }
    }

    /// Cached reference to the [`Self::peer`], if it is in cache.
    pub async fn peer_ref(&self) -> Option<PeerRef> {
        self.peers.get_ref(self.peer_id()).await
    }

    /// The peer where the callback query occured, if it is in cache.
    pub fn peer(&self) -> Option<&Peer> {
        self.peers.get(self.peer_id())
    }

    /// The [`Self::sender`]'s identifier.
    pub fn sender_id(&self) -> PeerId {
        PeerId::user(match &self.raw {
            tl::enums::Update::BotCallbackQuery(update) => update.user_id,
            tl::enums::Update::InlineBotCallbackQuery(update) => update.user_id,
            _ => unreachable!(),
        })
    }

    /// Cached reference to the [`Self::sender`], if it is in cache.
    pub async fn sender_ref(&self) -> Option<PeerRef> {
        self.peers.get_ref(self.sender_id()).await
    }

    /// The user who sent this callback query, if it is in cache.
    pub fn sender(&self) -> Option<&Peer> {
        self.peers.get(self.sender_id())
    }

    /// They binary payload data contained by the inline button which was pressed.
    ///
    /// This data cannot be faked by the client, since Telegram will only accept "button presses"
    /// on data that actually existed in the buttons of the message, so you do not need to perform
    /// any sanity checks.
    ///
    /// *Trivia*: it used to be possible to fake the callback data, but a server-side check was
    /// added circa 2018 to prevent malicious clients from doing so.
    pub fn data(&self) -> &[u8] {
        match &self.raw {
            tl::enums::Update::BotCallbackQuery(update) => update.data.as_deref().unwrap_or(&[]),
            tl::enums::Update::InlineBotCallbackQuery(update) => {
                update.data.as_deref().unwrap_or(&[])
            }
            _ => unreachable!(),
        }
    }

    /// Whether the callback query was generated from an inline message.
    pub fn is_from_inline(&self) -> bool {
        matches!(self.raw, tl::enums::Update::InlineBotCallbackQuery(_))
    }

    /// Load the `Message` that contains the pressed inline button.
    pub async fn load_message(&self) -> Result<Message, InvocationError> {
        let msg_id = match &self.raw {
            tl::enums::Update::BotCallbackQuery(update) => update.msg_id,
            _ => return Err(InvocationError::Dropped),
        };
        Ok(self
            .client
            .get_messages_by_id(
                self.peer_ref().await.ok_or(InvocationError::Dropped)?,
                &[msg_id],
            )
            .await?
            .pop()
            .unwrap()
            .unwrap())
    }

    /// Answer the callback query.
    pub fn answer(&self) -> Answer<'_> {
        let query_id = match &self.raw {
            tl::enums::Update::BotCallbackQuery(update) => update.query_id,
            tl::enums::Update::InlineBotCallbackQuery(update) => update.query_id,
            _ => unreachable!(),
        };
        Answer {
            request: tl::functions::messages::SetBotCallbackAnswer {
                alert: false,
                query_id,
                message: None,
                url: None,
                cache_time: 0,
            },
            query: self,
        }
    }
}

impl<'a> Answer<'a> {
    /// Configure the answer's text.
    ///
    /// The text will be displayed as a toast message (small popup which does not interrupt the
    /// user and fades on its own after a short period of time).
    pub fn text<T: Into<String>>(mut self, text: T) -> Self {
        self.request.message = Some(text.into());
        self.request.alert = false;
        self
    }

    /// For how long should the answer be considered valid. It will be cached by the client for
    /// the given duration, so subsequent callback queries with the same data will not reach the
    /// bot.
    pub fn cache_time(mut self, time: Duration) -> Self {
        self.request.cache_time = time.as_secs().try_into().unwrap_or(i32::MAX);
        self
    }

    /// Configure the answer's text.
    ///
    /// The text will be displayed as an alert (popup modal window with the text, which the user
    /// needs to close before performing other actions).
    pub fn alert<T: Into<String>>(mut self, text: T) -> Self {
        self.request.message = Some(text.into());
        self.request.alert = true;
        self
    }

    /// Send the answer back to Telegram, and then relayed to the user who pressed the inline
    /// button.
    pub async fn send(self) -> Result<(), InvocationError> {
        self.query.client.invoke(&self.request).await?;
        Ok(())
    }

    /// [`Self::send`] the answer, and also edit the message that contained the button.
    pub async fn edit<M: Into<InputMessage>>(self, new_message: M) -> Result<(), InvocationError> {
        self.query.client.invoke(&self.request).await?;
        let peer = self
            .query
            .peer_ref()
            .await
            .ok_or(InvocationError::Dropped)?;
        match &self.query.raw {
            tl::enums::Update::BotCallbackQuery(update) => {
                self.query
                    .client
                    .edit_message(peer, update.msg_id, new_message)
                    .await
            }
            tl::enums::Update::InlineBotCallbackQuery(update) => self
                .query
                .client
                .edit_inline_message(update.msg_id.clone(), new_message.into())
                .await
                .map(drop),
            _ => unreachable!(),
        }
    }

    /// [`Self::send`] the answer, and also respond in the peer where the button was clicked.
    pub async fn respond<M: Into<InputMessage>>(
        self,
        message: M,
    ) -> Result<Message, InvocationError> {
        self.query.client.invoke(&self.request).await?;
        let peer = self
            .query
            .peer_ref()
            .await
            .ok_or(InvocationError::Dropped)?;
        self.query.client.send_message(peer, message).await
    }

    /// [`Self::send`] the answer, and also reply to the message that contained the button.
    pub async fn reply<M: Into<InputMessage>>(
        self,
        message: M,
    ) -> Result<Message, InvocationError> {
        let msg_id = match &self.query.raw {
            tl::enums::Update::BotCallbackQuery(update) => update.msg_id,
            _ => return Err(InvocationError::Dropped),
        };
        self.query.client.invoke(&self.request).await?;
        let peer = self
            .query
            .peer_ref()
            .await
            .ok_or(InvocationError::Dropped)?;
        let message = message.into();
        self.query
            .client
            .send_message(peer, message.reply_to(Some(msg_id)))
            .await
    }
}

impl fmt::Debug for CallbackQuery {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("CallbackQuery")
            .field("data", &self.data())
            .field("sender", &self.sender())
            .field("peer", &self.peer())
            .finish()
    }
}
