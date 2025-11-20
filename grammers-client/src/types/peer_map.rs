// Copyright 2020 - developers of the `grammers` project.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// https://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or https://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.
use crate::types::{Peer, User};
use grammers_session::types::PeerId;
use grammers_tl_types as tl;
use std::collections::HashMap;
use std::sync::Arc;

/// Helper structure to efficiently retrieve peers via their peer.
///
/// A lot of responses include the peers related to them in the form of a list of users
/// and peers, making it annoying to extract a specific peer. This structure lets you
/// save those separate vectors in a single place and query them by using a `Peer`.
pub struct PeerMap {
    map: HashMap<PeerId, Peer>,
}

impl PeerMap {
    /// Create a new peer set.
    pub fn new<U, C>(users: U, peers: C) -> Arc<Self>
    where
        U: IntoIterator<Item = tl::enums::User>,
        C: IntoIterator<Item = tl::enums::Chat>,
    {
        Arc::new(Self {
            map: users
                .into_iter()
                .map(Peer::from_user)
                .chain(peers.into_iter().map(Peer::from_raw))
                .map(|peer| (peer.id(), peer))
                .collect(),
        })
    }

    /// Create a new empty peer set.
    pub fn empty() -> Arc<Self> {
        Arc::new(Self {
            map: HashMap::new(),
        })
    }

    /// Retrieve the full `Peer` object given its `PeerId`.
    pub fn get(&self, peer: PeerId) -> Option<&Peer> {
        self.map.get(&peer)
    }

    /// Take the full `Peer` object given its `PeerId` and remove it from the map.
    pub fn remove(&mut self, peer: PeerId) -> Option<Peer> {
        self.map.remove(&peer)
    }

    pub(crate) fn remove_user(&mut self, user_id: i64) -> Option<User> {
        self.map
            .remove(&PeerId::user(user_id))
            .map(|peer| match peer {
                Peer::User(user) => user,
                _ => unreachable!(),
            })
    }

    /// Iterate over the peers and peers in the map.
    pub fn iter(&self) -> impl Iterator<Item = (PeerId, &Peer)> {
        self.map.iter().map(|(k, v)| (*k, v))
    }

    /// Iterate over the peers in the map.
    pub fn iter_peers(&self) -> impl Iterator<Item = &Peer> {
        self.map.values()
    }
}
