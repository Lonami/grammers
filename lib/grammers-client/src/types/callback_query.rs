// Copyright 2020 - developers of the `grammers` project.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// https://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or https://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.
use crate::{types, Client};
use grammers_mtsender::InvocationError;
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
pub struct CallbackQuery {
    pub(crate) query: tl::types::UpdateBotCallbackQuery,
    pub(crate) client: Client,
    pub(crate) chats: Arc<types::ChatMap>,
}

/// A callback query answer builder.
///
/// It will be executed once `.await`-ed. Modifying it after polling it once will have no effect.
pub struct Answer<'a> {
    query: &'a mut CallbackQuery,
    request: tl::functions::messages::SetBotCallbackAnswer,
}

impl CallbackQuery {
    pub(crate) fn new(
        client: &Client,
        query: tl::types::UpdateBotCallbackQuery,
        chats: &Arc<types::ChatMap>,
    ) -> Self {
        Self {
            query,
            client: client.clone(),
            chats: chats.clone(),
        }
    }

    /// The user who sent this callback query.
    pub fn sender(&self) -> &types::Chat {
        self.chats
            .get(
                &tl::types::PeerUser {
                    user_id: self.query.user_id,
                }
                .into(),
            )
            .unwrap()
    }

    /// The chat where the callback query occured.
    pub fn chat(&self) -> &types::Chat {
        self.chats.get(&self.query.peer).unwrap()
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
        self.query.data.as_deref().unwrap()
    }

    /// Load the `Message` that contains the pressed inline button.
    pub async fn load_message(&self) -> Result<types::Message, InvocationError> {
        todo!()
    }

    /// Answer the callback query.
    pub fn answer(&mut self) -> Answer {
        Answer {
            request: tl::functions::messages::SetBotCallbackAnswer {
                alert: false,
                query_id: self.query.query_id,
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
    pub async fn edit(self, new_message: types::InputMessage) -> Result<(), InvocationError> {
        self.query.client.invoke(&self.request).await?;
        // TODO remove mut requirement (and clone)
        let chat = self.query.chat().clone();
        let msg_id = self.query.query.msg_id;
        self.query
            .client
            .edit_message(&chat, msg_id, new_message)
            .await
    }

    /// [`Self::send`] the answer, and also respond in the chat where the button was clicked.
    pub async fn respond(
        self,
        message: types::InputMessage,
    ) -> Result<types::Message, InvocationError> {
        self.query.client.invoke(&self.request).await?;
        // TODO remove mut requirement (and clone)
        let chat = self.query.chat().clone();
        self.query.client.send_message(&chat, message).await
    }

    /// [`Self::send`] the answer, and also reply to the message that contained the button.
    pub async fn reply(
        self,
        message: types::InputMessage,
    ) -> Result<types::Message, InvocationError> {
        self.query.client.invoke(&self.request).await?;
        // TODO remove mut requirement (and clone)
        let chat = self.query.chat().clone();
        self.query
            .client
            .send_message(&chat, message.reply_to(Some(self.query.query.msg_id)))
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
