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

use grammers_session::types::{PeerAuth, PeerId, PeerInfo, PeerRef};
use grammers_tl_types as tl;

pub use channel::Channel;
pub use group::Group;
pub use user::{Platform, RestrictionReason, User};

/// A peer.
///
/// Peers represent places where you can share messages with others.
///
/// * Private conversations with other people are treated as the peer of the user itself.
/// * Conversations in a group, whether it's private or public, are simply known as groups.
/// * Conversations where only administrators broadcast messages are known as channels.
#[derive(Clone, Debug)]
pub enum Peer {
    /// A [`User`].
    User(User),

    /// A [`Group`] chat.
    Group(Group),

    /// A broadcast [`Channel`].
    Channel(Channel),
}

impl Peer {
    pub(crate) fn from_user(user: tl::enums::User) -> Self {
        Self::User(User::from_raw(user))
    }

    pub fn from_raw(chat: tl::enums::Chat) -> Self {
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

    /// Return the unique identifier for this peer.
    ///
    /// Every account will see the same identifier for the same peer.
    ///
    /// This identifier will never change. However, small group chats may be migrated to
    /// megagroups. If this happens, both the old small group chat and the new megagroup
    /// exist as separate chats with different identifiers, but they are linked with a
    /// property.
    pub fn id(&self) -> PeerId {
        match self {
            Self::User(user) => PeerId::user(user.bare_id()),
            Self::Group(group) => group.id(),
            Self::Channel(channel) => PeerId::channel(channel.bare_id()),
        }
    }

    pub(crate) fn min(&self) -> bool {
        match self {
            Self::User(user) => user.min(),
            Self::Group(group) => group.min(),
            Self::Channel(channel) => channel.min(),
        }
    }

    pub(crate) fn auth(&self) -> PeerAuth {
        match self {
            Self::User(user) => user.auth(),
            Self::Group(group) => group.auth(),
            Self::Channel(channel) => channel.auth(),
        }
    }

    /// Return the name of this peer.
    ///
    /// For private conversations (users), this is their first name. For groups and channels,
    /// this is their title.
    ///
    /// The name will be `None` if the peer is inaccessible or if the account was deleted. It may
    /// also be `None` if you received it previously.
    pub fn name(&self) -> Option<&str> {
        match self {
            Self::User(user) => user.first_name(),
            Self::Group(group) => group.title(),
            Self::Channel(channel) => Some(channel.title()),
        }
    }

    /// Return the public @username of this peer, if any.
    ///
    /// The returned username does not contain the "@" prefix.
    ///
    /// Outside of the application, people may link to this user with one of Telegram's URLs, such
    /// as https://t.me/username.
    pub fn username(&self) -> Option<&str> {
        match self {
            Self::User(user) => user.username(),
            Self::Group(group) => group.username(),
            Self::Channel(channel) => channel.username(),
        }
    }

    /// Return collectible usernames of this peer, if any.
    ///
    /// The returned usernames do not contain the "@" prefix.
    ///
    /// Outside of the application, people may link to this user with one of its username, such
    /// as https://t.me/username.
    pub fn usernames(&self) -> Vec<&str> {
        match self {
            Self::User(user) => user.usernames(),
            Self::Group(group) => group.usernames(),
            Self::Channel(channel) => channel.usernames(),
        }
    }

    // Return the profile picture or chat photo of this peer, if any.
    pub fn photo(&self, big: bool) -> Option<crate::types::ChatPhoto> {
        let peer = PeerRef::from(self).into();
        match self {
            Self::User(user) => user.photo().map(|x| crate::types::ChatPhoto {
                raw: tl::enums::InputFileLocation::InputPeerPhotoFileLocation(
                    tl::types::InputPeerPhotoFileLocation {
                        big,
                        peer,
                        photo_id: x.photo_id,
                    },
                ),
            }),
            Self::Group(group) => group.photo().map(|x| crate::types::ChatPhoto {
                raw: tl::enums::InputFileLocation::InputPeerPhotoFileLocation(
                    tl::types::InputPeerPhotoFileLocation {
                        big,
                        peer,
                        photo_id: x.photo_id,
                    },
                ),
            }),
            Self::Channel(channel) => channel.photo().map(|x| crate::types::ChatPhoto {
                raw: tl::enums::InputFileLocation::InputPeerPhotoFileLocation(
                    tl::types::InputPeerPhotoFileLocation {
                        big,
                        peer,
                        photo_id: x.photo_id,
                    },
                ),
            }),
        }
    }
}

impl From<Peer> for PeerRef {
    #[inline]
    fn from(peer: Peer) -> Self {
        <Self as From<&Peer>>::from(&peer)
    }
}
impl<'a> From<&'a Peer> for PeerRef {
    fn from(peer: &'a Peer) -> Self {
        Self {
            id: peer.id(),
            auth: peer.auth(),
        }
    }
}

impl From<Peer> for PeerInfo {
    #[inline]
    fn from(peer: Peer) -> Self {
        <Self as From<&Peer>>::from(&peer)
    }
}
impl<'a> From<&'a Peer> for PeerInfo {
    fn from(peer: &'a Peer) -> Self {
        match peer {
            Peer::User(user) => <PeerInfo as From<&'a User>>::from(user),
            Peer::Group(group) => <PeerInfo as From<&'a Group>>::from(group),
            Peer::Channel(channel) => <PeerInfo as From<&'a Channel>>::from(channel),
        }
    }
}
