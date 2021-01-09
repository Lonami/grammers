// Copyright 2020 - developers of the `grammers` project.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// https://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or https://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

use chrono::{DateTime, NaiveDateTime, Utc};
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
