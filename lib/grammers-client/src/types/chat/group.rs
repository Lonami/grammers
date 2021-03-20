// Copyright 2020 - developers of the `grammers` project.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// https://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or https://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.
use grammers_tl_types as tl;
use std::fmt;

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

impl fmt::Debug for Group {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

// TODO it might be desirable to manually merge all the properties of the chat to avoid endless matching

impl Group {
    pub(crate) fn from_raw(chat: tl::enums::Chat) -> Self {
        use tl::enums::Chat as C;

        match chat {
            C::Empty(_) | C::Chat(_) | C::Forbidden(_) => Self(chat),
            C::Channel(ref channel) => {
                if channel.broadcast {
                    panic!("tried to create megagroup channel from broadcast");
                } else {
                    Self(chat)
                }
            }
            C::ChannelForbidden(ref channel) => {
                if channel.broadcast {
                    panic!("tried to create megagroup channel from broadcast");
                } else {
                    Self(chat)
                }
            }
        }
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
        use tl::enums::Chat as C;

        match &self.0 {
            C::Empty(chat) => tl::types::InputPeerChat { chat_id: chat.id }.into(),
            C::Chat(chat) => tl::types::InputPeerChat { chat_id: chat.id }.into(),
            C::Forbidden(chat) => tl::types::InputPeerChat { chat_id: chat.id }.into(),
            C::Channel(chat) => tl::types::InputPeerChannel {
                channel_id: chat.id,
                // TODO don't unwrap_or 0
                access_hash: chat.access_hash.unwrap_or(0),
            }
            .into(),
            C::ChannelForbidden(chat) => tl::types::InputPeerChannel {
                channel_id: chat.id,
                access_hash: chat.access_hash,
            }
            .into(),
        }
    }

    pub(crate) fn to_input_channel(&self) -> Option<tl::enums::InputChannel> {
        use tl::enums::Chat as C;

        match &self.0 {
            C::Empty(_) | C::Chat(_) | C::Forbidden(_) => None,
            C::Channel(chat) => Some(
                tl::types::InputChannel {
                    channel_id: chat.id,
                    // TODO don't unwrap_or 0
                    access_hash: chat.access_hash.unwrap_or(0),
                }
                .into(),
            ),
            C::ChannelForbidden(chat) => Some(
                tl::types::InputChannel {
                    channel_id: chat.id,
                    access_hash: chat.access_hash,
                }
                .into(),
            ),
        }
    }

    pub(crate) fn to_chat_id(&self) -> Option<i32> {
        use tl::enums::Chat as C;

        match &self.0 {
            C::Empty(chat) => Some(chat.id),
            C::Chat(chat) => Some(chat.id),
            C::Forbidden(chat) => Some(chat.id),
            C::Channel(_) | C::ChannelForbidden(_) => None,
        }
    }

    /// Return the unique identifier for this group.
    ///
    /// Note that if this group is migrated to a megagroup, both this group and the new one will
    /// exist as separate chats, with different identifiers.
    pub fn id(&self) -> i32 {
        use tl::enums::Chat;

        match &self.0 {
            Chat::Empty(chat) => chat.id,
            Chat::Chat(chat) => chat.id,
            Chat::Forbidden(chat) => chat.id,
            Chat::Channel(chat) => chat.id,
            Chat::ChannelForbidden(chat) => chat.id,
        }
    }

    pub(crate) fn access_hash(&self) -> Option<i64> {
        use tl::enums::Chat;

        match &self.0 {
            Chat::Empty(_) => None,
            Chat::Chat(_) => None,
            Chat::Forbidden(_) => None,
            Chat::Channel(chat) => chat.access_hash,
            Chat::ChannelForbidden(chat) => Some(chat.access_hash),
        }
    }

    /// Return the title of this group.
    ///
    /// The title may be the empty string if the group is not accessible.
    pub fn title(&self) -> &str {
        use tl::enums::Chat;

        match &self.0 {
            Chat::Empty(_) => "",
            Chat::Chat(chat) => chat.title.as_str(),
            Chat::Forbidden(chat) => chat.title.as_str(),
            Chat::Channel(chat) => chat.title.as_str(),
            Chat::ChannelForbidden(chat) => chat.title.as_str(),
        }
    }

    /// Returns true if this group is a megagroup (also known as supergroups).
    ///
    /// In case inner type of group is Channel, that means it's a megagroup.
    pub fn is_megagroup(&self) -> bool {
        use tl::enums::Chat as C;

        match &self.0 {
            C::Empty(_) | C::Chat(_) | C::Forbidden(_) => false,
            C::Channel(_) | C::ChannelForbidden(_) => true,
        }
    }
}
