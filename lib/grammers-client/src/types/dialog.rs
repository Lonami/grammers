// Copyright 2020 - developers of the `grammers` project.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// https://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or https://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

use super::{Chat, ChatMap, Message, Peer};
use grammers_tl_types as tl;
use std::collections::HashMap;

#[derive(Debug, Clone)]
pub struct Dialog {
    pub raw: tl::enums::Dialog,
    pub chat: Chat,
    pub last_message: Option<Message>,
}

impl Dialog {
    pub(crate) fn new(
        dialog: tl::enums::Dialog,
        messages: &mut HashMap<Peer, Message>,
        chats: &ChatMap,
    ) -> Self {
        // TODO helper utils (ext trait?) to extract data from dialogs or messages
        let peer = match dialog {
            tl::enums::Dialog::Dialog(ref dialog) => &dialog.peer,
            tl::enums::Dialog::Folder(ref dialog) => &dialog.peer,
        };

        Self {
            chat: chats
                .get(peer)
                .expect("dialogs use an unknown peer")
                .clone(),
            last_message: messages.remove(&peer.into()),
            raw: dialog,
        }
    }

    pub fn chat(&self) -> &Chat {
        &self.chat
    }
}
