// Copyright 2020 - developers of the `grammers` project.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// https://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or https://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

use chrono::{DateTime, NaiveDateTime, Utc};
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

        LAST_ID.compare_and_swap(0, now, Ordering::SeqCst);
    }

    LAST_ID.fetch_add(1, Ordering::SeqCst)
}

pub(crate) fn generate_random_ids(n: usize) -> Vec<i64> {
    (0..n).map(|_| generate_random_id()).collect()
}

pub(crate) fn date(date: i32) -> Date {
    DateTime::<Utc>::from_utc(NaiveDateTime::from_timestamp(date as i64, 0), Utc)
}

pub(crate) fn extract_password_parameters(
    current_algo: &tl::enums::PasswordKdfAlgo,
) -> (&Vec<u8>, &Vec<u8>, &i32, &Vec<u8>) {
    let tl::types::PasswordKdfAlgoSha256Sha256Pbkdf2Hmacsha512iter100000Sha256ModPow { salt1, salt2, g, p } = match current_algo {
        tl::enums::PasswordKdfAlgo::Unknown => panic!("Unknown KDF (most likely, the client is outdated and does not support the specified KDF algorithm)"),
        tl::enums::PasswordKdfAlgo::Sha256Sha256Pbkdf2Hmacsha512iter100000Sha256ModPow(alg) => alg,
    };
    (salt1, salt2, g, p)
}
