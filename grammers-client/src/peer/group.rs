// Copyright 2020 - developers of the `grammers` project.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// https://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or https://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

use std::fmt;

use grammers_session::types::{PeerAuth, PeerId, PeerInfo, PeerRef};
use grammers_tl_types as tl;

use crate::Client;

/// A group chat.
///
/// Telegram's API internally distinguishes between "small group chats" and "megagroups", also
/// known as "supergroups" in the UI of Telegram applications.
///
/// Small group chats are the default, and offer less features than megagroups, but you can
/// join more of them. Certain actions in official clients, like setting a chat's username,
/// silently upgrade the chat to a megagroup.
#[derive(Clone)]
pub struct Group {
    pub raw: tl::enums::Chat,
    pub(crate) client: Client,
}

impl fmt::Debug for Group {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.raw.fmt(f)
    }
}

// TODO it might be desirable to manually merge all the properties of the chat to avoid endless matching

impl Group {
    pub fn from_raw(client: &Client, chat: tl::enums::Chat) -> Self {
        use tl::enums::Chat as C;

        match chat {
            C::Empty(_) | C::Chat(_) | C::Forbidden(_) => Self {
                raw: chat,
                client: client.clone(),
            },
            C::Channel(ref channel) => {
                if channel.broadcast {
                    panic!("tried to create megagroup channel from broadcast");
                } else {
                    Self {
                        raw: chat,
                        client: client.clone(),
                    }
                }
            }
            C::ChannelForbidden(ref channel) => {
                if channel.broadcast {
                    panic!("tried to create megagroup channel from broadcast");
                } else {
                    Self {
                        raw: chat,
                        client: client.clone(),
                    }
                }
            }
        }
    }

    /// Return the unique identifier for this group.
    ///
    /// Note that if this group is migrated to a megagroup, both this group and the new one will
    /// exist as separate chats, with different identifiers.
    pub fn id(&self) -> PeerId {
        use tl::enums::Chat;

        match &self.raw {
            Chat::Empty(chat) => PeerId::chat(chat.id),
            Chat::Chat(chat) => PeerId::chat(chat.id),
            Chat::Forbidden(chat) => PeerId::chat(chat.id),
            Chat::Channel(channel) => PeerId::channel(channel.id),
            Chat::ChannelForbidden(channel) => PeerId::channel(channel.id),
        }
    }

    /// Non-min auth stored in the group, if any.
    pub(crate) fn auth(&self) -> Option<PeerAuth> {
        use tl::enums::Chat;

        Some(match &self.raw {
            Chat::Empty(_) => PeerAuth::default(),
            Chat::Chat(_) => PeerAuth::default(),
            Chat::Forbidden(_) => PeerAuth::default(),
            Chat::Channel(channel) => {
                return channel
                    .access_hash
                    .filter(|_| !channel.min)
                    .map(PeerAuth::from_hash);
            }
            Chat::ChannelForbidden(channel) => PeerAuth::from_hash(channel.access_hash),
        })
    }

    /// Convert the group to its reference.
    ///
    /// This is only possible if the peer would be usable on all methods or if it is in the session cache.
    pub async fn to_ref(&self) -> Option<PeerRef> {
        let id = self.id();
        match self.auth() {
            Some(auth) => Some(PeerRef { id, auth }),
            None => self.client.0.session.peer_ref(id).await,
        }
    }

    /// Return the title of this group.
    ///
    /// The title may be the empty string if the group is not accessible.
    pub fn title(&self) -> Option<&str> {
        use tl::enums::Chat;

        match &self.raw {
            Chat::Empty(_) => None,
            Chat::Chat(chat) => Some(chat.title.as_str()),
            Chat::Forbidden(chat) => Some(chat.title.as_str()),
            Chat::Channel(channel) => Some(channel.title.as_str()),
            Chat::ChannelForbidden(channel) => Some(channel.title.as_str()),
        }
    }

    /// Return the public @username of this group, if any.
    ///
    /// The returned username does not contain the "@" prefix.
    ///
    /// Outside of the application, people may link to this user with one of Telegram's URLs, such
    /// as https://t.me/username.
    pub fn username(&self) -> Option<&str> {
        use tl::enums::Chat;

        match &self.raw {
            Chat::Empty(_) | Chat::Chat(_) | Chat::Forbidden(_) | Chat::ChannelForbidden(_) => None,
            Chat::Channel(channel) => channel.username.as_deref(),
        }
    }

    /// Return collectible usernames of this group, if any.
    ///
    /// The returned usernames do not contain the "@" prefix.
    ///
    /// Outside of the application, people may link to this user with one of its username, such
    /// as https://t.me/username.
    pub fn usernames(&self) -> Vec<&str> {
        use tl::enums::Chat;

        match &self.raw {
            Chat::Empty(_) | Chat::Chat(_) | Chat::Forbidden(_) | Chat::ChannelForbidden(_) => {
                Vec::new()
            }
            Chat::Channel(channel) => {
                channel
                    .usernames
                    .as_deref()
                    .map_or(Vec::new(), |usernames| {
                        usernames
                            .iter()
                            .map(|username| match username {
                                tl::enums::Username::Username(username) => {
                                    username.username.as_ref()
                                }
                            })
                            .collect()
                    })
            }
        }
    }

    // Return photo of this group, if any.
    pub fn photo(&self) -> Option<&tl::types::ChatPhoto> {
        match &self.raw {
            tl::enums::Chat::Empty(_)
            | tl::enums::Chat::Forbidden(_)
            | tl::enums::Chat::ChannelForbidden(_) => None,
            tl::enums::Chat::Chat(chat) => match &chat.photo {
                tl::enums::ChatPhoto::Empty => None,
                tl::enums::ChatPhoto::Photo(photo) => Some(photo),
            },
            tl::enums::Chat::Channel(channel) => match &channel.photo {
                tl::enums::ChatPhoto::Empty => None,
                tl::enums::ChatPhoto::Photo(photo) => Some(photo),
            },
        }
    }

    /// Returns true if this group is a megagroup (also known as supergroups).
    ///
    /// In case inner type of group is Channel, that means it's a megagroup.
    pub fn is_megagroup(&self) -> bool {
        use tl::enums::Chat as C;

        match &self.raw {
            C::Empty(_) | C::Chat(_) | C::Forbidden(_) => false,
            C::Channel(_) | C::ChannelForbidden(_) => true,
        }
    }
}

impl From<Group> for PeerInfo {
    #[inline]
    fn from(group: Group) -> Self {
        <Self as From<&Group>>::from(&group)
    }
}
impl<'a> From<&'a Group> for PeerInfo {
    fn from(group: &'a Group) -> Self {
        <Self as From<&'a tl::enums::Chat>>::from(&group.raw)
    }
}
