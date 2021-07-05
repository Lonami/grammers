// Copyright 2020 - developers of the `grammers` project.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// https://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or https://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.
mod channel;
mod group;
mod user;

use grammers_tl_types as tl;
use std::fmt;

pub use channel::Channel;
pub use group::Group;
pub use user::{Platform, RestrictionReason, User};

/// A chat.
///
/// Chats represent places where you can share messages with others.
///
/// * Private conversations with other people are treated as the chat of the user itself.
/// * Conversations in a group, whether it's private or public, are simply known as groups.
/// * Conversations where only administrators broadcast messages are known as channels.
#[derive(Clone, Debug)]
pub enum Chat {
    /// A [`User`].
    User(User),

    /// A [`Group`] chat.
    Group(Group),

    /// A broadcast [`Channel`].
    Channel(Channel),
}

#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(crate) enum PackedType {
    // The fancy bit pattern may enable some optimizations.
    // * 2nd bit for tl::enums::Peer::User
    // * 3rd bit for tl::enums::Peer::Chat
    // * 6th bit for tl::enums::Peer::Channel
    User = 0b0000_0010,
    Bot = 0b0000_0011,
    Chat = 0b0000_0100,
    Megagroup = 0b0010_1000,
    Broadcast = 0b0011_0000,
    Gigagroup = 0b0011_1000,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
/// A packed chat
pub struct PackedChat {
    pub(crate) ty: PackedType,
    pub(crate) id: i32,
    pub(crate) access_hash: Option<i64>,
}

impl PackedChat {
    /// Serialize the packed chat into a new buffer and return its bytes.
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut res = if let Some(access_hash) = self.access_hash {
            let mut res = vec![0; 14];
            res[6..14].copy_from_slice(&access_hash.to_le_bytes());
            res
        } else {
            vec![0; 6]
        };
        res[0] = self.ty as u8;
        res[1] = res.len() as u8;
        res[2..6].copy_from_slice(&self.id.to_le_bytes());
        res
    }

    /// Deserialize the buffer into a packed chat
    pub fn from_bytes(buf: &[u8]) -> Result<Self, ()> {
        if buf.len() != 6 && buf.len() != 14 {
            return Err(());
        }
        if buf[1] as usize != buf.len() {
            return Err(());
        }
        let ty = match buf[0] {
            0b0000_0010 => PackedType::User,
            0b0000_0011 => PackedType::Bot,
            0b0000_0100 => PackedType::Chat,
            0b0010_1000 => PackedType::Megagroup,
            0b0011_0000 => PackedType::Broadcast,
            0b0011_1000 => PackedType::Gigagroup,
            _ => return Err(()),
        };
        let id = i32::from_le_bytes([buf[2], buf[3], buf[4], buf[5]]);
        let access_hash = if buf[1] == 14 {
            Some(i64::from_le_bytes([
                buf[6], buf[7], buf[8], buf[9], buf[10], buf[11], buf[12], buf[13],
            ]))
        } else {
            None
        };
        Ok(Self {
            ty,
            id,
            access_hash,
        })
    }

    /// Unpack this into a `Chat` that can be used in requests.
    pub fn unpack(&self) -> Chat {
        // TODO this isn't ideal, because it's quite wasteful
        // create instances of the smallest representations that work for us to avoid providing all fields
        match self.ty {
            PackedType::User | PackedType::Bot => {
                let mut user = User::from_raw(tl::types::UserEmpty { id: self.id }.into());
                user.0.access_hash = self.access_hash;
                Chat::User(user)
            }
            PackedType::Chat => {
                Chat::Group(Group::from_raw(tl::types::ChatEmpty { id: self.id }.into()))
            }
            PackedType::Megagroup => Chat::Group(Group::from_raw(
                tl::types::ChannelForbidden {
                    id: self.id,
                    broadcast: false,
                    megagroup: true,
                    access_hash: self.access_hash.unwrap_or(0),
                    title: String::new(),
                    until_date: None,
                }
                .into(),
            )),
            PackedType::Broadcast | PackedType::Gigagroup => Chat::Channel(Channel::from_raw(
                tl::types::ChannelForbidden {
                    id: self.id,
                    broadcast: true,
                    megagroup: false,
                    access_hash: self.access_hash.unwrap_or(0),
                    title: String::new(),
                    until_date: None,
                }
                .into(),
            )),
        }
    }
}

impl fmt::Display for PackedType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::User => "User",
            Self::Bot => "Bot",
            Self::Chat => "Group",
            Self::Megagroup => "Supergroup",
            Self::Broadcast => "Channel",
            Self::Gigagroup => "BroadcastGroup",
        })
    }
}

impl fmt::Display for PackedChat {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "PackedChat::{}({})", self.ty, self.id)
    }
}

impl Chat {
    pub(crate) fn from_user(user: tl::enums::User) -> Self {
        Self::User(User::from_raw(user))
    }

    pub(crate) fn from_chat(chat: tl::enums::Chat) -> Self {
        use tl::enums::Chat as C;

        match chat {
            C::Empty(_) | C::Chat(_) | C::Forbidden(_) => Self::Group(Group::from_raw(chat)),
            C::Channel(ref channel) => {
                if channel.broadcast {
                    Self::Channel(Channel::from_raw(chat))
                } else {
                    Self::Group(Group::from_raw(chat))
                }
            }
            C::ChannelForbidden(ref channel) => {
                if channel.broadcast {
                    Self::Channel(Channel::from_raw(chat))
                } else {
                    Self::Group(Group::from_raw(chat))
                }
            }
        }
    }

    pub(crate) fn to_peer(&self) -> tl::enums::Peer {
        match self {
            Self::User(user) => user.to_peer(),
            Self::Group(group) => group.to_peer(),
            Self::Channel(channel) => channel.to_peer(),
        }
    }

    pub(crate) fn to_input_peer(&self) -> tl::enums::InputPeer {
        match self {
            Self::User(user) => user.to_input_peer(),
            Self::Group(group) => group.to_input_peer(),
            Self::Channel(channel) => channel.to_input_peer(),
        }
    }

    pub(crate) fn to_input_user(&self) -> Option<tl::enums::InputUser> {
        match self {
            Self::User(user) => Some(user.to_input()),
            Self::Group(_) => None,
            Self::Channel(_) => None,
        }
    }

    pub(crate) fn to_input_channel(&self) -> Option<tl::enums::InputChannel> {
        match self {
            Self::User(_) => None,
            Self::Group(group) => group.to_input_channel(),
            Self::Channel(channel) => Some(channel.to_input()),
        }
    }

    pub(crate) fn to_chat_id(&self) -> Option<i32> {
        match self {
            Self::User(_) => None,
            Self::Group(group) => group.to_chat_id(),
            Self::Channel(_) => None,
        }
    }

    /// Return the unique identifier for this chat.
    ///
    /// Every account will see the same identifier for the same chat.
    ///
    /// This identifier will never change. However, small group chats may be migrated to
    /// megagroups. If this happens, both the old small group chat and the new megagroup
    /// exist as separate chats with different identifiers, but they are linked with a
    /// property.
    pub fn id(&self) -> i32 {
        match self {
            Self::User(user) => user.id(),
            Self::Group(group) => group.id(),
            Self::Channel(channel) => channel.id(),
        }
    }

    fn access_hash(&self) -> Option<i64> {
        match self {
            Self::User(user) => user.access_hash(),
            Self::Group(group) => group.access_hash(),
            Self::Channel(channel) => channel.access_hash(),
        }
    }

    /// Return the name of this chat.
    ///
    /// For private conversations (users), this is their first name. For groups and channels,
    /// this is their title.
    ///
    /// The name may be empty if the chat is inaccessible or if the account was deleted.
    pub fn name(&self) -> &str {
        match self {
            Self::User(user) => user.first_name(),
            Self::Group(group) => group.title(),
            Self::Channel(channel) => channel.title(),
        }
    }

    /// Pack this chat into a smaller representation that can be loaded later.
    pub fn pack(&self) -> PackedChat {
        let ty = match self {
            Self::User(user) => {
                if user.is_bot() {
                    PackedType::Bot
                } else {
                    PackedType::User
                }
            }
            Self::Group(chat) => {
                if chat.is_megagroup() {
                    PackedType::Megagroup
                } else {
                    PackedType::Chat
                }
            }
            Self::Channel(channel) => {
                if channel.0.gigagroup {
                    PackedType::Gigagroup
                } else {
                    PackedType::Broadcast
                }
            }
        };

        PackedChat {
            ty,
            id: self.id(),
            access_hash: self.access_hash(),
        }
    }
}
