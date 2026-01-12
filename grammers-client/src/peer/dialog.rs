// Copyright 2020 - developers of the `grammers` project.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// https://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or https://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

use grammers_session::types::{PeerId, PeerRef};
use grammers_tl_types as tl;

use super::{Peer, PeerMap};
use crate::Client;
use crate::message::Message;
use crate::utils::peer_from_message;

/// An entry in the list of "chats".
///
/// All conversations with history, even if the history has been cleared,
/// as long as the dialog itself has not been deleted, are present as dialogs.
///
/// Bot accounts do not have dialogs per-se and thus cannot fetch them.
///
/// Dialogs of users continue to exist even if the user has deleted their account.
/// The same is true for small group chats, but not of large group chats and channels.
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
        peers: PeerMap,
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
            last_message: message.map(|m| {
                Message::from_raw(
                    client,
                    m,
                    Some(PeerRef {
                        id: peer.id(),
                        auth: peer.auth().unwrap(),
                    }),
                    peers.handle(),
                )
            }),
            peer,
            raw: dialog,
        }
    }

    /// The [`Self::peer`]'s identifier.
    pub fn peer_id(&self) -> PeerId {
        self.peer.id()
    }

    /// Cached reference to the [`Self::peer`].
    pub fn peer_ref(&self) -> PeerRef {
        PeerRef {
            id: self.peer.id(),
            auth: self.peer.auth().unwrap(),
        }
    }

    /// The peer represented by this dialog.
    pub fn peer(&self) -> &Peer {
        &self.peer
    }
}
