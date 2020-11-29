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
use std::sync::Arc;

/// Hashable `Peer`.
#[derive(Hash, PartialEq, Eq)]
pub(crate) enum Peer {
    User(i32),
    Chat(i32),
    Channel(i32),
}

impl From<&tl::enums::Peer> for Peer {
    fn from(peer: &tl::enums::Peer) -> Self {
        use tl::enums::Peer::*;

        match peer {
            User(user) => Self::User(user.user_id),
            Chat(chat) => Self::Chat(chat.chat_id),
            Channel(channel) => Self::Channel(channel.channel_id),
        }
    }
}

/// Helper structure to efficiently retrieve entities via their peer.
///
/// A lot of responses include the entities related to them in the form of a list of users
/// and chats, making it annoying to extract a specific entity. This structure lets you
/// save those separate vectors in a single place and query them by using a `Peer`.
pub struct EntitySet {
    map: HashMap<Peer, Entity>,
}

impl EntitySet {
    /// Create a new entity set.
    pub fn new(users: Vec<tl::enums::User>, chats: Vec<tl::enums::Chat>) -> Arc<Self> {
        use tl::enums::{Chat, User};

        Arc::new(Self {
            map: users
                .into_iter()
                .filter_map(|user| match user {
                    User::User(user) => Some(Entity::User(user)),
                    User::Empty(_) => None,
                })
                .chain(chats.into_iter().filter_map(|chat| match chat {
                    Chat::Empty(_) => None,
                    Chat::Chat(chat) => Some(Entity::Chat(chat)),
                    Chat::Forbidden(_) => None,
                    Chat::Channel(channel) => Some(Entity::Channel(channel)),
                    Chat::ChannelForbidden(_) => None,
                    // TODO *Forbidden have some info which may be relevant at times
                    // currently ignored for simplicity
                }))
                .map(|entity| ((&entity.peer()).into(), entity))
                .collect(),
        })
    }

    /// Create a new empty entity set.
    pub fn empty() -> Arc<Self> {
        Arc::new(Self {
            map: HashMap::new(),
        })
    }

    /// Retrieve the full `Entity` object given its `Peer`.
    pub fn get<'a, 'b>(&'a self, peer: &'b tl::enums::Peer) -> Option<&'a Entity> {
        self.map.get(&peer.into())
    }
}
