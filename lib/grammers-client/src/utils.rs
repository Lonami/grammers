// Copyright 2020 - developers of the `grammers` project.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// https://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or https://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

use crate::types;
use chrono::{DateTime, NaiveDateTime, TimeZone, Utc};
use grammers_session::{PackedChat, PackedType};
use grammers_tl_types as tl;
use std::sync::atomic::{AtomicI64, Ordering};
use std::time::SystemTime;

// This atomic isn't for anything critical, just to generate unique IDs without locks.
// The worst that can happen if the load and store orderings are wrong is that the IDs
// are not actually unique which could confuse some of the API results.
static LAST_ID: AtomicI64 = AtomicI64::new(0);

pub(crate) type Date = DateTime<Utc>;

/// Generate a "random" ID suitable for sending messages or media.
pub(crate) fn generate_random_id() -> i64 {
    if LAST_ID.load(Ordering::SeqCst) == 0 {
        let now = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .expect("system time is before epoch")
            .as_nanos() as i64;

        LAST_ID
            .compare_exchange(0, now, Ordering::SeqCst, Ordering::SeqCst)
            .unwrap();
    }

    LAST_ID.fetch_add(1, Ordering::SeqCst)
}

pub(crate) fn generate_random_ids(n: usize) -> Vec<i64> {
    (0..n).map(|_| generate_random_id()).collect()
}

pub(crate) fn date(date: i32) -> Date {
    Utc.from_utc_datetime(
        &NaiveDateTime::from_timestamp_opt(date as i64, 0).expect("date out of range"),
    )
}

pub(crate) fn extract_password_parameters(
    current_algo: &tl::enums::PasswordKdfAlgo,
) -> (&Vec<u8>, &Vec<u8>, &Vec<u8>, &i32) {
    let tl::types::PasswordKdfAlgoSha256Sha256Pbkdf2Hmacsha512iter100000Sha256ModPow { salt1, salt2, p, g } = match current_algo {
        tl::enums::PasswordKdfAlgo::Unknown => panic!("Unknown KDF (most likely, the client is outdated and does not support the specified KDF algorithm)"),
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
    let get_packed = || {
        let (id, ty) = match peer {
            tl::enums::Peer::User(user) => (user.user_id, PackedType::User),
            tl::enums::Peer::Chat(chat) => (chat.chat_id, PackedType::Chat),
            tl::enums::Peer::Channel(channel) => (channel.channel_id, PackedType::Broadcast),
        };
        client
            .0
            .state
            .read()
            .unwrap()
            .chat_hashes
            .get(id)
            .unwrap_or(PackedChat {
                ty,
                id,
                access_hash: None,
            })
    };

    match map.get(peer).cloned() {
        Some(mut chat) => {
            // As a best-effort, attempt to replace any `min` `access_hash` with the non-`min`
            // version. The `min` hash is only usable to download profile photos (if the user
            // tried to pack it for later use, like sending a message, it would fail).
            if let Some((min, access_hash)) = chat.get_min_hash_ref() {
                let packed = get_packed();
                if let Some(ah) = packed.access_hash {
                    *access_hash = ah;
                    *min = false;
                }
            }
            chat
        }
        None => types::Chat::unpack(get_packed()),
    }
}
