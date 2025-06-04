// Copyright 2020 - developers of the `grammers` project.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// https://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or https://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

use crate::{Client, InputMessage, types};
use grammers_mtsender::InvocationError;
use grammers_session::State;
use grammers_tl_types as tl;
use std::convert::TryInto;
use std::fmt;
use std::sync::Arc;
use std::time::Duration;

/// Represents a callback query update, which occurs when a user presses one of the bot's inline
/// callback buttons.
///
/// You should always [`CallbackQuery::answer`] these queries, even if you have no data to display
/// to the user, because otherwise they will think the bot is non-responsive (the button spinner
/// will timeout).
#[derive(Clone)]
pub struct CallbackQuery {
    pub raw: tl::enums::Update,
    pub state: State,
    pub(crate) client: Client,
    pub(crate) chats: Arc<types::ChatMap>,
}

/// A callback query answer builder.
///
/// It will be executed once `.await`-ed. Modifying it after polling it once will have no effect.
pub struct Answer<'a> {
    query: &'a CallbackQuery,
    request: tl::functions::messages::SetBotCallbackAnswer,
}

impl CallbackQuery {
    /// The user who sent this callback query.
    pub fn sender(&self) -> &types::Chat {
        let user_id = match &self.raw {
            tl::enums::Update::BotCallbackQuery(update) => update.user_id,
            tl::enums::Update::InlineBotCallbackQuery(update) => update.user_id,
            _ => unreachable!(),
        };
        self.chats
            .get(&tl::types::PeerUser { user_id }.into())
            .unwrap()
    }

    /// The chat where the callback query occured.
    pub fn chat(&self) -> &types::Chat {
        let peer = match &self.raw {
            tl::enums::Update::BotCallbackQuery(update) => &update.peer,
            tl::enums::Update::InlineBotCallbackQuery(update) => {
                &tl::enums::Peer::User(tl::types::PeerUser {
                    user_id: update.user_id,
                })
            }
            _ => unreachable!(),
        };
        self.chats.get(peer).unwrap()
    }

    /// They binary payload data contained by the inline button which was pressed.
    ///
    /// This data cannot be faked by the client, since Telegram will only accept "button presses"
    /// on data that actually existed in the buttons of the message, so you do not need to perform
    /// any sanity checks.
    ///
    /// > Trivia: it used to be possible to fake the callback data, but a server-side check was
    /// > added circa 2018 to prevent malicious clients from doing so.
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
    pub async fn load_message(&self) -> Result<types::Message, InvocationError> {
        let msg_id = match &self.raw {
            tl::enums::Update::BotCallbackQuery(update) => update.msg_id,
            _ => return Err(InvocationError::Dropped),
        };
        Ok(self
            .client
            .get_messages_by_id(self.chat(), &[msg_id])
            .await?
            .pop()
            .unwrap()
            .unwrap())
    }

    /// Answer the callback query.
    pub fn answer(&self) -> Answer {
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
        let chat = self.query.chat();
        match &self.query.raw {
            tl::enums::Update::BotCallbackQuery(update) => {
                self.query
                    .client
                    .edit_message(chat, update.msg_id, new_message)
                    .await
            }
            tl::enums::Update::InlineBotCallbackQuery(update) => self
                .query
                .client
                .edit_inline_message(update.msg_id.clone(), new_message)
                .await
                .map(drop),
            _ => unreachable!(),
        }
    }

    /// [`Self::send`] the answer, and also respond in the chat where the button was clicked.
    pub async fn respond<M: Into<InputMessage>>(
        self,
        message: M,
    ) -> Result<types::Message, InvocationError> {
        self.query.client.invoke(&self.request).await?;
        let chat = self.query.chat();
        self.query.client.send_message(chat, message).await
    }

    /// [`Self::send`] the answer, and also reply to the message that contained the button.
    pub async fn reply<M: Into<InputMessage>>(
        self,
        message: M,
    ) -> Result<types::Message, InvocationError> {
        let msg_id = match &self.query.raw {
            tl::enums::Update::BotCallbackQuery(update) => update.msg_id,
            _ => return Err(InvocationError::Dropped),
        };
        self.query.client.invoke(&self.request).await?;
        let chat = self.query.chat();
        let message = message.into();
        self.query
            .client
            .send_message(chat, message.reply_to(Some(msg_id)))
            .await
    }
}

impl fmt::Debug for CallbackQuery {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("CallbackQuery")
            .field("data", &self.data())
            .field("sender", &self.sender())
            .field("chat", &self.chat())
            .finish()
    }
}
