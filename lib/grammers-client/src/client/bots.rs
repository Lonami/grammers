// Copyright 2020 - developers of the `grammers` project.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// https://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or https://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.
use crate::client::messages::parse_mention_entities;
use crate::utils::generate_random_id;
use crate::Client;
use crate::{types::IterBuffer, InputMessage};
pub use grammers_mtsender::{AuthorizationError, InvocationError};
use grammers_session::PackedChat;
use grammers_tl_types as tl;

const MAX_LIMIT: usize = 50;

pub struct InlineResult {
    client: Client,
    query_id: i64,
    result: tl::enums::BotInlineResult,
}

pub type InlineResultIter = IterBuffer<tl::functions::messages::GetInlineBotResults, InlineResult>;

impl InlineResult {
    /// Send this inline result to the specified chat.
    // TODO return the produced message
    pub async fn send<C: Into<PackedChat>>(&self, chat: C) -> Result<(), InvocationError> {
        self.client
            .invoke(&tl::functions::messages::SendInlineBotResult {
                silent: false,
                background: false,
                clear_draft: false,
                hide_via: false,
                peer: chat.into().to_input_peer(),
                reply_to: None,
                random_id: generate_random_id(),
                query_id: self.query_id,
                id: self.id().to_string(),
                schedule_date: None,
                send_as: None,
                quick_reply_shortcut: None,
            })
            .await
            .map(drop)
    }

    /// The ID for this result.
    pub fn id(&self) -> &str {
        use tl::enums::BotInlineResult::*;

        match &self.result {
            Result(r) => &r.id,
            BotInlineMediaResult(r) => &r.id,
        }
    }

    /// The title for this result, if any.
    pub fn title(&self) -> Option<&String> {
        use tl::enums::BotInlineResult::*;

        match &self.result {
            Result(r) => r.title.as_ref(),
            BotInlineMediaResult(r) => r.title.as_ref(),
        }
    }
}

impl InlineResultIter {
    fn new(client: &Client, bot: PackedChat, query: &str) -> Self {
        Self::from_request(
            client,
            MAX_LIMIT,
            tl::functions::messages::GetInlineBotResults {
                bot: bot.to_input_user_lossy(),
                peer: tl::enums::InputPeer::Empty,
                geo_point: None,
                query: query.to_string(),
                offset: String::new(),
            },
        )
    }

    /// Indicate the bot the chat where this inline query will be sent to.
    ///
    /// Some bots use this information to return different results depending on the type of the
    /// chat, and some even "need" it to give useful results.
    pub fn chat<C: Into<PackedChat>>(mut self, chat: C) -> Self {
        self.request.peer = chat.into().to_input_peer();
        self
    }

    /// Return the next `InlineResult` from the internal buffer, filling the buffer previously if
    /// it's empty.
    ///
    /// Returns `None` if the `limit` is reached or there are no results left.
    pub async fn next(&mut self) -> Result<Option<InlineResult>, InvocationError> {
        if let Some(result) = self.next_raw() {
            return result;
        }

        let tl::enums::messages::BotResults::Results(tl::types::messages::BotResults {
            query_id,
            next_offset,
            results,
            ..
        }) = self.client.invoke(&self.request).await?;

        if let Some(offset) = next_offset {
            self.request.offset = offset;
        } else {
            self.last_chunk = true;
        }

        let client = self.client.clone();
        self.buffer
            .extend(results.into_iter().map(|r| InlineResult {
                client: client.clone(),
                query_id,
                result: r,
            }));

        Ok(self.pop_item())
    }
}

/// Method implementations related to dealing with bots.
impl Client {
    /// Perform an inline query to the specified bot.
    ///
    /// The query text may be empty.
    ///
    /// The return value is used like any other async iterator, by repeatedly calling `next`.
    ///
    /// Executing the query will fail if the input chat does not actually represent a bot account
    /// supporting inline mode.
    ///
    /// # Examples
    ///
    /// ```
    /// # async fn f(bot: grammers_client::types::User, client: grammers_client::Client) -> Result<(), Box<dyn std::error::Error>> {
    /// // This is equivalent to writing `@bot inline query` in a Telegram app.
    /// let mut inline_results = client.inline_query(&bot, "inline query");
    ///
    /// while let Some(result) = inline_results.next().await? {
    ///     println!("{}", result.title().unwrap());
    /// }
    /// # Ok(())
    /// # }
    /// ```
    pub fn inline_query<C: Into<PackedChat>>(&self, bot: C, query: &str) -> InlineResultIter {
        InlineResultIter::new(self, bot.into(), query)
    }

    pub async fn edit_inline_message<M: Into<InputMessage>>(
        &self,
        message_id: tl::enums::InputBotInlineMessageId,
        input_message: M,
    ) -> Result<bool, InvocationError> {
        let message: InputMessage = input_message.into();
        let entities = parse_mention_entities(self, message.entities);
        let result = self
            .invoke(&tl::functions::messages::EditInlineBotMessage {
                id: message_id,
                message: Some(message.text),
                media: message.media,
                entities,
                no_webpage: !message.link_preview,
                reply_markup: message.reply_markup,
                invert_media: false,
            })
            .await?;
        Ok(result)
    }
}
