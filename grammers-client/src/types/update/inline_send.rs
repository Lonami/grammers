// Copyright 2020 - developers of the `grammers` project.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// https://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or https://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

use crate::types::{Peer, User};
use crate::{Client, InputMessage, PeerMap};
use grammers_mtsender::InvocationError;
use grammers_session::types::{PeerAuth, PeerId, PeerRef};
use grammers_session::updates::State;
use grammers_tl_types as tl;
use std::fmt;
use std::sync::Arc;

/// Represents an update of user choosing the result of inline query and sending it to a peer.
///
/// To receive this update, "Inline Feedback" under "Bot Settings" must be enabled via [@BotFather](https://t.me/BotFather).
#[derive(Clone)]
pub struct InlineSend {
    pub raw: tl::enums::Update,
    pub state: State,
    pub(crate) client: Client,
    pub(crate) peers: Arc<PeerMap>,
}

impl InlineSend {
    fn update(&self) -> &tl::types::UpdateBotInlineSend {
        match &self.raw {
            tl::enums::Update::BotInlineSend(update) => update,
            _ => unreachable!(),
        }
    }

    /// The query that was used to obtain the result.
    pub fn text(&self) -> &str {
        self.update().query.as_str()
    }

    /// The reference to the user that chose the result.
    pub fn sender_ref(&self) -> PeerRef {
        let id = PeerId::user(self.update().user_id);
        match self.client.0.session.peer(id) {
            Some(info) => info.into(),
            None => PeerRef {
                id,
                auth: PeerAuth::default(),
            },
        }
    }

    /// The user that chose the result.
    pub fn sender(&self) -> &User {
        match self.peers.get(self.sender_ref().id).unwrap() {
            Peer::User(user) => user,
            _ => unreachable!(),
        }
    }

    /// The unique identifier for the result that was chosen
    pub fn result_id(&self) -> &str {
        self.update().id.as_str()
    }

    /// Identifier of sent inline message.
    /// Available only if there is an inline keyboard attached.
    /// Will be also received in callback queries and can be used to edit the message.
    pub fn message_id(&self) -> Option<tl::enums::InputBotInlineMessageId> {
        self.update().msg_id.clone()
    }

    /// Edits this inline message.
    ///
    /// **This method will return Ok(None) if message id is None (e.g. if an inline keyboard is not attached)**
    pub async fn edit_message(
        &self,
        input_message: impl Into<InputMessage>,
    ) -> Result<Option<bool>, InvocationError> {
        let msg_id = match self.update().msg_id.clone() {
            None => return Ok(None),
            Some(msg_id) => msg_id,
        };

        Ok(Some(
            self.client
                .edit_inline_message(msg_id, input_message)
                .await?,
        ))
    }
}

impl fmt::Debug for InlineSend {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("InlineSend")
            .field("text", &self.text())
            .field("sender", &self.sender())
            .field("result_id", &self.result_id())
            .field("message_id", &self.message_id())
            .finish()
    }
}
