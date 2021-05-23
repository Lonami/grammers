// Copyright 2020 - developers of the `grammers` project.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// https://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or https://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.
use crate::types::{Chat, IterBuffer, User};
use crate::utils::generate_random_id;
use crate::Client;
pub use grammers_mtsender::{AuthorizationError, InvocationError};
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
    pub async fn send(&mut self, chat: &Chat) -> Result<(), InvocationError> {
        self.client
            .invoke(&tl::functions::messages::SendInlineBotResult {
                silent: false,
                background: false,
                clear_draft: false,
                hide_via: false,
                peer: chat.to_input_peer(),
                reply_to_msg_id: None,
                random_id: generate_random_id(),
                query_id: self.query_id,
                id: self.id().to_string(),
                schedule_date: None,
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
    fn new(client: &Client, bot: &User, query: &str) -> Self {
        Self::from_request(
            client,
            MAX_LIMIT,
            tl::functions::messages::GetInlineBotResults {
                bot: bot.to_input(),
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
    pub fn chat(mut self, chat: &Chat) -> Self {
        self.request.peer = chat.to_input_peer();
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
        }) = dbg!(self.client.invoke(&self.request).await?);

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
    /// # Examples
    ///
    /// ```
    /// # async fn f(bot: grammers_client::types::User, mut client: grammers_client::Client) -> Result<(), Box<dyn std::error::Error>> {
    /// // This is equivalent to writing `@bot inline query` in a Telegram app.
    /// let mut inline_results = client.inline_query(&bot, "inline query");
    ///
    /// while let Some(result) = inline_results.next().await? {
    ///     println!("{}", result.title().unwrap());
    /// }
    /// # Ok(())
    /// # }
    /// ```
    pub fn inline_query(&self, bot: &User, query: &str) -> InlineResultIter {
        InlineResultIter::new(self, bot, query)
    }
}
