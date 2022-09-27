// Copyright 2020 - developers of the `grammers` project.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// https://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or https://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.
use grammers_session::{PackedChat, PackedType};
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

    /// Return the unique identifier for this group.
    ///
    /// Note that if this group is migrated to a megagroup, both this group and the new one will
    /// exist as separate chats, with different identifiers.
    pub fn id(&self) -> i64 {
        use tl::enums::Chat;

        match &self.0 {
            Chat::Empty(chat) => chat.id,
            Chat::Chat(chat) => chat.id,
            Chat::Forbidden(chat) => chat.id,
            Chat::Channel(chat) => chat.id,
            Chat::ChannelForbidden(chat) => chat.id,
        }
    }

    /// Pack this group into a smaller representation that can be loaded later.
    pub fn pack(&self) -> PackedChat {
        use tl::enums::Chat;
        let (id, access_hash) = match &self.0 {
            Chat::Empty(chat) => (chat.id, None),
            Chat::Chat(chat) => (chat.id, None),
            Chat::Forbidden(chat) => (chat.id, None),
            Chat::Channel(chat) => (chat.id, chat.access_hash),
            Chat::ChannelForbidden(chat) => (chat.id, Some(chat.access_hash)),
        };

        PackedChat {
            ty: if self.is_megagroup() {
                PackedType::Megagroup
            } else {
                PackedType::Chat
            },
            id,
            access_hash,
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
    
    /// Return the access_hash of this group.
    pub fn access_hash(&self) -> Option<i64> {
        use tl::enums::Chat as C;

        match &self.0 {
            C::Empty(_) | C::Chat(_) | C::Forbidden(_) => None,
            C::Channel(channel) => channel.access_hash,
            C::ChannelForbidden(channel) => Some(channel.access_hash),
        }
    }

    /// Return the broadcast of this group.
    pub fn broadcast(&self) -> bool {
        use tl::enums::Chat as C;

        match &self.0 {
            C::Empty(_) | C::Chat(_) | C::Forbidden(_) => false,
            C::Channel(channel) => channel.broadcast,
            C::ChannelForbidden(channel) => channel.broadcast,
        }
    }

    /// Return the username of this group.
    pub fn username(&self) -> Option<String> {
        use tl::enums::Chat as C;

        match &self.0 {
            C::Empty(_) | C::Chat(_) | C::Forbidden(_) | C::ChannelForbidden(_) => None,
            C::Channel(channel) => channel.username.clone(),
        }
    }
    
    /// Return the min of this group.
    pub fn is_min(&self) -> Option<bool> {
        use tl::enums::Chat as C;

        match &self.0 {
            C::Empty(_) | C::Chat(_) | C::Forbidden(_) | C::ChannelForbidden(_) => None,
            C::Channel(c) => Some(c.min),
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

impl From<Group> for PackedChat {
    fn from(chat: Group) -> Self {
        chat.pack()
    }
}

impl From<&Group> for PackedChat {
    fn from(chat: &Group) -> Self {
        chat.pack()
    }
}
