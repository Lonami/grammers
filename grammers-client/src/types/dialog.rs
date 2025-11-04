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

use super::{Message, Peer, PeerMap};
use grammers_session::types::PeerId;
use grammers_tl_types as tl;

#[derive(Debug, Clone)]
pub struct Dialog {
    pub raw: tl::enums::Dialog,
    pub peer: Peer,
    pub last_message: Option<Message>,
}

impl Dialog {
    pub(crate) fn new(
        client: &Client,
        dialog: tl::enums::Dialog,
        messages: &mut Vec<tl::enums::Message>,
        peers: &Arc<PeerMap>,
    ) -> Self {
        // TODO helper utils (ext trait?) to extract data from dialogs or messages
        let peer_id = match dialog {
            tl::enums::Dialog::Dialog(ref dialog) => dialog.peer.clone().into(),
            tl::enums::Dialog::Folder(ref dialog) => dialog.peer.clone().into(),
        };

        let peer = peers
            .get(peer_id)
            .expect("dialogs use an unknown peer")
            .clone();

        let message = messages
            .iter()
            .position(|m| peer_from_message(m).is_some_and(|p| PeerId::from(p) == peer_id))
            .map(|i| messages.swap_remove(i));

        Self {
            last_message: message
                .map(|m| Message::from_raw(client, m, Some((&peer).into()), peers)),
            peer,
            raw: dialog,
        }
    }

    pub fn peer(&self) -> &Peer {
        &self.peer
    }
}
