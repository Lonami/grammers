// Copyright 2020 - developers of the `grammers` project.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// https://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or https://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.
use super::{CallbackQuery, ChatMap, InlineQuery, Message};
use crate::Client;
use grammers_tl_types as tl;
use std::sync::Arc;

#[non_exhaustive]
#[derive(Debug)]
pub enum Update {
    /// Occurs whenever a new text message or a message with media is produced.
    NewMessage(Message),
    /// Occurs when Telegram calls back into your bot because an inline callback button was
    /// pressed.
    CallbackQuery(CallbackQuery),
    /// Occurs whenever you sign in as a bot and a user sends an inline query such as
    /// `@bot query`.
    InlineQuery(InlineQuery),
}

impl Update {
    pub(crate) fn new(
        client: &Client,
        update: tl::enums::Update,
        chats: &Arc<ChatMap>,
    ) -> Option<Self> {
        match update {
            tl::enums::Update::NewMessage(tl::types::UpdateNewMessage { message, .. }) => {
                Message::new(client, message, chats).map(Self::NewMessage)
            }
            tl::enums::Update::NewChannelMessage(tl::types::UpdateNewChannelMessage {
                message,
                ..
            }) => Message::new(client, message, chats).map(Self::NewMessage),
            tl::enums::Update::BotCallbackQuery(query) => Some(Self::CallbackQuery(
                CallbackQuery::new(client, query, chats),
            )),
            tl::enums::Update::BotInlineQuery(query) => {
                Some(Self::InlineQuery(InlineQuery::new(client, query, chats)))
            }
            _ => None,
        }
    }
}
