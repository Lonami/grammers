// Copyright 2020 - developers of the `grammers` project.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// https://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or https://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

use crate::types::{Chat, User};
use crate::ChatMap;
use grammers_tl_types as tl;
use grammers_tl_types::enums::InputBotInlineMessageId;
use std::fmt;
use std::sync::Arc;

#[derive(Clone)]
pub struct InlineSend {
    raw: tl::types::UpdateBotInlineSend,
    chats: Arc<ChatMap>,
}

impl InlineSend {
    pub fn from_raw(query: tl::types::UpdateBotInlineSend, chats: &Arc<ChatMap>) -> Self {
        Self {
            raw: query,
            chats: chats.clone(),
        }
    }

    /// The query that was used to obtain the result.
    pub fn text(&self) -> &str {
        self.raw.query.as_str()
    }

    /// The user that chose the result.
    pub fn sender(&self) -> &User {
        match self
            .chats
            .get(
                &tl::types::PeerUser {
                    user_id: self.raw.user_id,
                }
                .into(),
            )
            .unwrap()
        {
            Chat::User(user) => user,
            _ => unreachable!(),
        }
    }

    /// The unique identifier for the result that was chosen
    pub fn result_id(&self) -> &str {
        self.raw.id.as_str()
    }

    // TODO: maybe custom InputBotInlineMessage and edit method

    /// Identifier of sent inline message.
    /// Available only if there is an inline keyboard attached.
    /// Will be also received in callback queries and can be used to edit the message.
    pub fn msg_id(&self) -> Option<InputBotInlineMessageId> {
        self.raw.msg_id.clone()
    }
}

impl fmt::Debug for InlineSend {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("InlineSend")
            .field("text", &self.text())
            .field("sender", &self.sender())
            .field("result_id", &self.result_id())
            .field("msg_id", &self.msg_id())
            .finish()
    }
}
