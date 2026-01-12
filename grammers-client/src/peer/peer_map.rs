// Copyright 2020 - developers of the `grammers` project.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// https://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or https://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

use std::collections::HashMap;
use std::sync::Arc;

use grammers_session::Session;
use grammers_session::types::{PeerId, PeerRef};

use crate::peer::{Peer, User};

/// Helper structure to efficiently retrieve peers via their peer.
///
/// A lot of responses include the peers related to them in the form of a list of users
/// and peers, making it annoying to extract a specific peer. This structure lets you
/// save those separate vectors in a single place and query them by using a `Peer`.
///
/// While this type derives `Clone` for convenience, it is recommended to use
/// [`PeerMap::handle`] instead to signal that it is a cheap clone.
#[derive(Clone)]
pub struct PeerMap {
    pub(crate) map: Arc<HashMap<PeerId, Peer>>,
    pub(crate) session: Arc<dyn Session>,
}

impl PeerMap {
    /// Retrieve the full `Peer` object given its `PeerId`.
    pub fn get(&self, peer: PeerId) -> Option<&Peer> {
        self.map.get(&peer)
    }

    /// Retrieve a non-min `PeerRef` from either the in-memory cache or the session.
    pub async fn get_ref(&self, peer: PeerId) -> Option<PeerRef> {
        match self.map.get(&peer) {
            Some(peer) => peer.to_ref().await,
            None => self.session.peer_ref(peer).await,
        }
    }

    /// Take the full `Peer` object given its `PeerId`.
    ///
    /// The peer will be removed from the map if there are no other strong references to it.
    pub fn take(&mut self, peer: PeerId) -> Option<Peer> {
        match Arc::get_mut(&mut self.map) {
            Some(map) => map.remove(&peer),
            None => self.get(peer).cloned(),
        }
    }

    pub(crate) fn take_user(&mut self, user_id: i64) -> Option<User> {
        self.take(PeerId::user(user_id)).map(|peer| match peer {
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

    /// Return a new strong reference to the map and session contained within.
    pub fn handle(&self) -> Self {
        Self {
            map: Arc::clone(&self.map),
            session: Arc::clone(&self.session),
        }
    }
}
