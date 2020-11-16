// Copyright 2020 - developers of the `grammers` project.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// https://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or https://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

use super::{Entity, EntitySet, Message};
use grammers_tl_types as tl;

pub struct Dialog {
    pub dialog: tl::enums::Dialog,
    pub entity: Entity,
    pub last_message: Option<Message>,
}

impl Dialog {
    pub(crate) fn new(
        dialog: tl::enums::Dialog,
        messages: &[Message],
        entities: &EntitySet,
    ) -> Self {
        // TODO helper utils (ext trait?) to extract data from dialogs or messages
        let peer = match dialog {
            tl::enums::Dialog::Dialog(ref dialog) => &dialog.peer,
            tl::enums::Dialog::Folder(ref dialog) => &dialog.peer,
        };

        Self {
            entity: entities
                .get(peer)
                .expect("dialogs use an unknown peer")
                .clone(),
            last_message: messages.iter().find_map(|m| {
                if &m.msg.peer_id == peer {
                    Some(m.clone())
                } else {
                    None
                }
            }),
            dialog,
        }
    }

    pub fn title(&self) -> &str {
        self.entity.name()
    }

    pub fn id(&self) -> i32 {
        self.entity.id()
    }

    pub fn peer(&self) -> tl::enums::Peer {
        self.entity.peer()
    }

    pub fn input_peer(&self) -> tl::enums::InputPeer {
        self.entity.input_peer()
    }
}
