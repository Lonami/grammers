// Copyright 2020 - developers of the `grammers` project.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// https://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or https://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

use crate::types;
use chrono::{DateTime, Utc};
use grammers_session::{Peer, PeerInfo, PeerKind};
use grammers_tl_types as tl;
use std::sync::atomic::{AtomicI64, Ordering};
use std::thread;
use std::time::SystemTime;

// This atomic isn't for anything critical, just to generate unique IDs without locks.
// The worst that can happen if the load and store orderings are wrong is that the IDs
// are not actually unique which could confuse some of the API results.
static LAST_ID: AtomicI64 = AtomicI64::new(0);

/// Generate a "random" ID suitable for sending messages or media.
pub(crate) fn generate_random_id() -> i64 {
    while LAST_ID.load(Ordering::SeqCst) == 0 {
        let now = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .expect("system time is before epoch")
            .as_nanos() as i64;

        if LAST_ID
            .compare_exchange(0, now, Ordering::SeqCst, Ordering::SeqCst)
            .is_err()
        {
            thread::yield_now();
        }
    }

    LAST_ID.fetch_add(1, Ordering::SeqCst)
}

pub(crate) fn generate_random_ids(n: usize) -> Vec<i64> {
    (0..n).map(|_| generate_random_id()).collect()
}

pub(crate) fn date(date: i32) -> DateTime<Utc> {
    DateTime::<Utc>::from_timestamp(date as i64, 0).expect("date out of range")
}

pub(crate) fn extract_password_parameters(
    current_algo: &tl::enums::PasswordKdfAlgo,
) -> (&Vec<u8>, &Vec<u8>, &Vec<u8>, &i32) {
    let tl::types::PasswordKdfAlgoSha256Sha256Pbkdf2Hmacsha512iter100000Sha256ModPow {
        salt1,
        salt2,
        p,
        g,
    } = match current_algo {
        tl::enums::PasswordKdfAlgo::Unknown => panic!(
            "Unknown KDF (most likely, the client is outdated and does not support the specified KDF algorithm)"
        ),
        tl::enums::PasswordKdfAlgo::Sha256Sha256Pbkdf2Hmacsha512iter100000Sha256ModPow(alg) => alg,
    };
    (salt1, salt2, p, g)
}

/// Get a `Chat`, no matter what.
///
/// If necessary, `access_hash` of `0` will be returned, but *something* will be returned.
///
/// If the `Chat` is `min`, attempt to update its `access_hash` to the non-`min` version.
pub(crate) fn always_find_entity(
    peer: &tl::enums::Peer,
    map: &types::ChatMap,
    client: &crate::Client,
) -> types::Chat {
    let peer_info = || {
        let peer = Peer::from(peer.clone());
        client
            .0
            .session
            .peer(peer)
            .unwrap_or_else(|| match peer.kind() {
                PeerKind::User | PeerKind::UserSelf => PeerInfo::User {
                    id: peer.id(),
                    hash: Some(peer.auth()),
                    bot: None,
                    is_self: None,
                },
                PeerKind::Chat => PeerInfo::Chat { id: peer.id() },
                PeerKind::Channel => PeerInfo::Channel {
                    id: peer.id(),
                    hash: Some(peer.auth()),
                    kind: None,
                },
            })
    };

    match map.get(peer).cloned() {
        Some(mut chat) => {
            // As a best-effort, attempt to replace any `min` `access_hash` with the non-`min`
            // version. The `min` hash is only usable to download profile photos (if the user
            // tried to pack it for later use, like sending a message, it would fail).
            if let Some((min, access_hash)) = chat.get_min_hash_ref() {
                if let Some(ah) = peer_info().hash() {
                    *access_hash = ah;
                    *min = false;
                }
            }
            chat
        }
        None => types::Chat::unpack(peer_info().into()),
    }
}

pub fn peer_from_message(message: &tl::enums::Message) -> Option<&tl::enums::Peer> {
    match &message {
        tl::enums::Message::Empty(message) => message.peer_id.as_ref(),
        tl::enums::Message::Message(message) => Some(&message.peer_id),
        tl::enums::Message::Service(message) => Some(&message.peer_id),
    }
}
