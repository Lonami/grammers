// Copyright 2020 - developers of the `grammers` project.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// https://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or https://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.
use crate::types::{Chat, User};
use grammers_session::{AMBIENT_AUTH, Peer};
use grammers_tl_types as tl;
use std::collections::HashMap;
use std::sync::Arc;

/// Helper structure to efficiently retrieve chats via their peer.
///
/// A lot of responses include the chats related to them in the form of a list of users
/// and chats, making it annoying to extract a specific chat. This structure lets you
/// save those separate vectors in a single place and query them by using a `Peer`.
pub struct ChatMap {
    map: HashMap<Peer, Chat>,
}

impl ChatMap {
    /// Create a new chat set.
    pub fn new<U, C>(users: U, chats: C) -> Arc<Self>
    where
        U: IntoIterator<Item = tl::enums::User>,
        C: IntoIterator<Item = tl::enums::Chat>,
    {
        Arc::new(Self {
            map: users
                .into_iter()
                .map(Chat::from_user)
                .chain(chats.into_iter().map(Chat::from_raw))
                .map(|chat| (chat.peer().with_auth(AMBIENT_AUTH), chat))
                .collect(),
        })
    }

    /// Create a new empty chat set.
    pub fn empty() -> Arc<Self> {
        Arc::new(Self {
            map: HashMap::new(),
        })
    }

    pub fn single(chat: Chat) -> Arc<Self> {
        let mut map = HashMap::new();
        map.insert(chat.peer(), chat);
        Arc::new(Self { map })
    }

    /// Retrieve the full `Chat` object given its `Peer`.
    pub fn get(&self, peer: &tl::enums::Peer) -> Option<&Chat> {
        self.map.get(&peer.clone().into())
    }

    /// Take the full `Chat` object given its `Peer` and remove it from the map.
    pub fn remove(&mut self, peer: &tl::enums::Peer) -> Option<Chat> {
        self.map.remove(&peer.clone().into())
    }

    pub(crate) fn remove_user(&mut self, user_id: i64) -> Option<User> {
        self.map
            .remove(&Peer::user(user_id))
            .map(|chat| match chat {
                Chat::User(user) => user,
                _ => unreachable!(),
            })
    }

    /// Iterate over the peers and chats in the map.
    pub fn iter(&self) -> impl Iterator<Item = (Peer, &Chat)> {
        self.map.iter().map(|(k, v)| (*k, v))
    }

    /// Iterate over the chats in the map.
    pub fn iter_chats(&self) -> impl Iterator<Item = &Chat> {
        self.map.values()
    }
}
