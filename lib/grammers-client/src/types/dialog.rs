// Copyright 2020 - developers of the `grammers` project.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// https://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or https://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

use super::{Chat, EntitySet, Message, Peer};
use grammers_tl_types as tl;
use std::collections::HashMap;

pub struct Dialog {
    pub dialog: tl::enums::Dialog,
    pub chat: Chat,
    pub last_message: Option<Message>,
}

impl Dialog {
    pub(crate) fn new(
        dialog: tl::enums::Dialog,
        messages: &mut HashMap<Peer, Message>,
        entities: &EntitySet,
    ) -> Self {
        // TODO helper utils (ext trait?) to extract data from dialogs or messages
        let peer = match dialog {
            tl::enums::Dialog::Dialog(ref dialog) => &dialog.peer,
            tl::enums::Dialog::Folder(ref dialog) => &dialog.peer,
        };

        Self {
            chat: entities
                .get(peer)
                .expect("dialogs use an unknown peer")
                .clone(),
            last_message: messages.remove(&peer.into()),
            dialog,
        }
    }

    pub fn title(&self) -> &str {
        todo!()
    }

    pub fn id(&self) -> i32 {
        todo!()
    }

    pub fn peer(&self) -> tl::enums::Peer {
        self.chat.to_peer()
    }

    pub fn input_peer(&self) -> tl::enums::InputPeer {
        self.chat.to_input_peer()
    }
}
