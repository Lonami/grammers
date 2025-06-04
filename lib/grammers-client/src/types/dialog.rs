// Copyright 2020 - developers of the `grammers` project.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// https://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or https://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

use std::sync::Arc;

use crate::Client;
use crate::utils::peer_from_message;

use super::{Chat, ChatMap, Message};
use grammers_tl_types as tl;

#[derive(Debug, Clone)]
pub struct Dialog {
    pub raw: tl::enums::Dialog,
    pub chat: Chat,
    pub last_message: Option<Message>,
}

impl Dialog {
    pub(crate) fn new(
        client: &Client,
        dialog: tl::enums::Dialog,
        messages: &mut Vec<tl::enums::Message>,
        chats: &Arc<ChatMap>,
    ) -> Self {
        // TODO helper utils (ext trait?) to extract data from dialogs or messages
        let peer = match dialog {
            tl::enums::Dialog::Dialog(ref dialog) => &dialog.peer,
            tl::enums::Dialog::Folder(ref dialog) => &dialog.peer,
        };

        let chat = chats
            .get(peer)
            .expect("dialogs use an unknown peer")
            .clone();

        let message = messages
            .iter()
            .position(|m| peer_from_message(m).is_some_and(|p| p == peer))
            .map(|i| messages.swap_remove(i));

        Self {
            chat,
            last_message: message.map(|m| Message::from_raw(client, m, Some(peer.clone()), chats)),
            raw: dialog,
        }
    }

    pub fn chat(&self) -> &Chat {
        &self.chat
    }
}
