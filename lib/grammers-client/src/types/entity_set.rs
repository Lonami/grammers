// Copyright 2020 - developers of the `grammers` project.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// https://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or https://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.
use crate::types::Entity;
use grammers_tl_types as tl;
use std::collections::HashMap;
use std::ops::Index;

/// Hashable `Peer`.
#[derive(Hash, PartialEq, Eq)]
enum Peer {
    User(i32),
    Chat(i32),
    Channel(i32),
}

pub enum MaybeBorrowedVec<'a, T> {
    Borrowed(&'a [T]),
    Owned(Vec<T>),
}

/// Helper structure to efficiently retrieve entities via their peer.
///
/// A lot of responses include the entities related to them in the form of a list of users
/// and chats, making it annoying to extract a specific entity. This structure lets you
/// save those separate vectors in a single place and query them by using a `Peer`.
pub struct EntitySet<'a> {
    users: MaybeBorrowedVec<'a, tl::enums::User>,
    chats: MaybeBorrowedVec<'a, tl::enums::Chat>,

    // Because we can't store references to other fields, we instead store the index
    map: HashMap<Peer, usize>,
}

fn build_map(users: &[tl::enums::User], chats: &[tl::enums::Chat]) -> HashMap<Peer, usize> {
    let mut map = HashMap::new();

    for (i, user) in users.into_iter().enumerate() {
        match user {
            tl::enums::User::User(user) => {
                map.insert(Peer::User(user.id), i);
            }
            tl::enums::User::Empty(_) => {}
        }
    }

    for (i, chat) in chats.into_iter().enumerate() {
        let i = users.len() + i;

        match chat {
            tl::enums::Chat::Chat(chat) => {
                map.insert(Peer::Chat(chat.id), i);
            }
            tl::enums::Chat::Forbidden(chat) => {
                map.insert(Peer::Chat(chat.id), i);
            }
            tl::enums::Chat::Channel(channel) => {
                map.insert(Peer::Channel(channel.id), i);
            }
            tl::enums::Chat::ChannelForbidden(channel) => {
                map.insert(Peer::Channel(channel.id), i);
            }
            tl::enums::Chat::Empty(_) => {}
        }
    }

    map
}

impl<T> MaybeBorrowedVec<'_, T> {
    fn len(&self) -> usize {
        match self {
            MaybeBorrowedVec::Borrowed(slice) => slice.len(),
            MaybeBorrowedVec::Owned(vec) => vec.len(),
        }
    }
}

impl<T> Index<usize> for MaybeBorrowedVec<'_, T> {
    type Output = T;

    fn index(&self, index: usize) -> &Self::Output {
        match self {
            MaybeBorrowedVec::Borrowed(slice) => &slice[index],
            MaybeBorrowedVec::Owned(vec) => &vec[index],
        }
    }
}

impl<'a> EntitySet<'a> {
    /// Create a borrowed entity set.
    ///
    /// Useful when you can't or don't want to take ownership of the lists.
    pub fn new_borrowed(users: &'a [tl::enums::User], chats: &'a [tl::enums::Chat]) -> Self {
        let map = build_map(users, chats);
        Self {
            users: MaybeBorrowedVec::Borrowed(users),
            chats: MaybeBorrowedVec::Borrowed(chats),
            map,
        }
    }

    /// Create a new owned entity set.
    ///
    /// Useful when you need to pass or hold on to the instance.
    pub fn new_owned(users: Vec<tl::enums::User>, chats: Vec<tl::enums::Chat>) -> Self {
        let map = build_map(&users, &chats);
        Self {
            users: MaybeBorrowedVec::Owned(users),
            chats: MaybeBorrowedVec::Owned(chats),
            map,
        }
    }

    /// Create a new empty entity set.
    ///
    /// Useful when there is no information known about any entities.
    pub fn empty() -> Self {
        Self {
            users: MaybeBorrowedVec::Owned(Vec::new()),
            chats: MaybeBorrowedVec::Owned(Vec::new()),
            map: HashMap::new(),
        }
    }

    /// Retrieve the full `Entity` object given its `Peer`.
    pub fn get(&self, peer: &tl::enums::Peer) -> Option<Entity> {
        let key = match peer {
            tl::enums::Peer::User(tl::types::PeerUser { user_id }) => (Peer::User(*user_id)),
            tl::enums::Peer::Chat(tl::types::PeerChat { chat_id }) => (Peer::Chat(*chat_id)),
            tl::enums::Peer::Channel(tl::types::PeerChannel { channel_id }) => {
                Peer::Channel(*channel_id)
            }
        };

        self.map
            .get(&key)
            .map(|&index| {
                if index < self.users.len() {
                    match self.users[index] {
                        tl::enums::User::User(ref user) => Some(Entity::User(user)),
                        tl::enums::User::Empty(_) => None,
                    }
                } else {
                    match self.chats[index - self.users.len()] {
                        tl::enums::Chat::Chat(ref chat) => Some(Entity::Chat(chat)),
                        tl::enums::Chat::Forbidden(_) => None,
                        tl::enums::Chat::Channel(ref channel) => Some(Entity::Channel(channel)),
                        tl::enums::Chat::ChannelForbidden(_) => None,
                        tl::enums::Chat::Empty(_) => None,
                    }
                }
            })
            .flatten()
    }
}
