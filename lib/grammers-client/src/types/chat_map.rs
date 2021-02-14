// Copyright 2020 - developers of the `grammers` project.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// https://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or https://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.
use crate::types::{Chat, User};
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

/// Helper structure to efficiently retrieve chats via their peer.
///
/// A lot of responses include the chats related to them in the form of a list of users
/// and chats, making it annoying to extract a specific chat. This structure lets you
/// save those separate vectors in a single place and query them by using a `Peer`.
pub struct ChatMap {
    map: HashMap<Peer, Chat>,
}

/// In-memory chat cache, mapping peers to their respective access hashes.
pub(crate) struct ChatHashCache {
    users: HashMap<i32, i64>,
    channels: HashMap<i32, i64>,
    self_id: Option<i32>,
    self_bot: bool,
}

impl ChatMap {
    /// Create a new chat set.
    pub fn new(users: Vec<tl::enums::User>, chats: Vec<tl::enums::Chat>) -> Arc<Self> {
        Arc::new(Self {
            map: users
                .into_iter()
                .map(|user| Chat::from_user(user))
                .chain(chats.into_iter().map(|chat| Chat::from_chat(chat)))
                .map(|chat| ((&chat.to_peer()).into(), chat))
                .collect(),
        })
    }

    /// Create a new empty chat set.
    pub fn empty() -> Arc<Self> {
        Arc::new(Self {
            map: HashMap::new(),
        })
    }

    pub fn single(chat: &Chat) -> Arc<Self> {
        let mut map = HashMap::new();
        map.insert((&chat.to_peer()).into(), chat.clone());
        Arc::new(Self { map })
    }

    /// Retrieve the full `Chat` object given its `Peer`.
    pub fn get<'a, 'b>(&'a self, peer: &'b tl::enums::Peer) -> Option<&'a Chat> {
        self.map.get(&peer.into())
    }

    /// Take the full `Chat` object given its `Peer` and remove it from the map.
    pub fn remove(&mut self, peer: &tl::enums::Peer) -> Option<Chat> {
        self.map.remove(&peer.into())
    }

    pub(crate) fn remove_user(&mut self, user_id: i32) -> Option<User> {
        self.map
            .remove(&Peer::User(user_id))
            .map(|chat| match chat {
                Chat::User(user) => user,
                _ => unreachable!(),
            })
    }
}

impl ChatHashCache {
    pub(crate) fn new() -> Self {
        Self {
            users: HashMap::new(),
            channels: HashMap::new(),
            self_id: None,
            self_bot: false,
        }
    }

    pub(crate) fn self_id(&self) -> i32 {
        self.self_id
            .expect("tried to query self_id before it's known")
    }

    pub(crate) fn is_self_bot(&self) -> bool {
        self.self_bot
    }

    pub(crate) fn contains_user(&self, user_id: i32) -> bool {
        self.users.contains_key(&user_id)
    }

    pub(crate) fn get_input_channel(&self, channel_id: i32) -> Option<tl::enums::InputChannel> {
        self.channels.get(&channel_id).map(|&access_hash| {
            tl::types::InputChannel {
                channel_id,
                access_hash,
            }
            .into()
        })
    }
}
