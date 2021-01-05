// Copyright 2020 - developers of the `grammers` project.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// https://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or https://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.
use super::{ChatMap, Message};
use crate::ClientHandle;
use grammers_tl_types as tl;
use std::sync::Arc;

#[non_exhaustive]
pub enum Update {
    /// Occurs whenever a new text message or a message with media is produced.
    NewMessage(Message),
}

impl Update {
    pub(crate) fn new(
        client: &ClientHandle,
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
            _ => None,
        }
    }
}
