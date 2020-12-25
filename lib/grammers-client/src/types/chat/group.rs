// Copyright 2020 - developers of the `grammers` project.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// https://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or https://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.
use grammers_tl_types as tl;

/// A group chat.
///
/// Telegram's API internally distinguishes between "small group chats" and "megagroups", also
/// known as "supergroups" in the UI of Telegram applications.
///
/// Small group chats are the default, and offer less features than megagroups, but you can
/// join more of them. Certain actions in official clients, like setting a chat's username,
/// silently upgrade the chat to a megagroup.
#[derive(Clone)]
pub struct Group(tl::enums::Chat);

impl Group {
    pub(crate) fn from_raw(_chat: tl::enums::Chat) -> Self {
        todo!()
    }

    pub(crate) fn to_peer(&self) -> tl::enums::Peer {
        use tl::enums::Chat;

        match &self.0 {
            Chat::Empty(chat) => tl::types::PeerChat { chat_id: chat.id }.into(),
            Chat::Chat(chat) => tl::types::PeerChat { chat_id: chat.id }.into(),
            Chat::Forbidden(chat) => tl::types::PeerChat { chat_id: chat.id }.into(),
            Chat::Channel(chat) => tl::types::PeerChannel {
                channel_id: chat.id,
            }
            .into(),
            Chat::ChannelForbidden(chat) => tl::types::PeerChannel {
                channel_id: chat.id,
            }
            .into(),
        }
    }

    pub(crate) fn to_input_peer(&self) -> tl::enums::InputPeer {
        todo!()
    }
}
