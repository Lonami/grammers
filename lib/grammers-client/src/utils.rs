// Copyright 2020 - developers of the `grammers` project.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// https://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or https://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

use chrono::{DateTime, NaiveDateTime, Utc};
use grammers_tl_types as tl;
use log::trace;
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

pub(crate) struct Mutex<T: ?Sized> {
    name: &'static str,
    mutex: std::sync::Mutex<T>,
}

pub(crate) struct MutexGuard<'a, T: ?Sized> {
    name: &'static str,
    reason: &'static str,
    guard: std::sync::MutexGuard<'a, T>,
}

impl<T> Mutex<T> {
    pub fn new(name: &'static str, value: T) -> Self {
        Self {
            name,
            mutex: std::sync::Mutex::new(value),
        }
    }

    pub fn lock(&self, reason: &'static str) -> MutexGuard<T> {
        trace!("locking {} for {}", self.name, reason);
        MutexGuard {
            name: self.name,
            reason,
            guard: self.mutex.lock().unwrap(),
        }
    }
}

impl<T: ?Sized + std::fmt::Debug> std::fmt::Debug for Mutex<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.mutex.fmt(f)
    }
}

impl<T: ?Sized> std::ops::Deref for MutexGuard<'_, T> {
    type Target = T;

    fn deref(&self) -> &T {
        self.guard.deref()
    }
}

impl<T: ?Sized> std::ops::DerefMut for MutexGuard<'_, T> {
    fn deref_mut(&mut self) -> &mut T {
        self.guard.deref_mut()
    }
}

impl<'a, T: ?Sized> Drop for MutexGuard<'a, T> {
    fn drop(&mut self) {
        trace!("unlocking {} for {}", self.name, self.reason);
    }
}

pub(crate) struct AsyncMutex<T: ?Sized> {
    name: &'static str,
    mutex: tokio::sync::Mutex<T>,
}

pub(crate) struct AsyncMutexGuard<'a, T: ?Sized> {
    name: &'static str,
    reason: &'static str,
    guard: tokio::sync::MutexGuard<'a, T>,
}

impl<T> AsyncMutex<T> {
    pub fn new(name: &'static str, value: T) -> Self {
        Self {
            name,
            mutex: tokio::sync::Mutex::new(value),
        }
    }

    pub fn try_lock<'a>(
        &'a self,
        reason: &'static str,
    ) -> Result<AsyncMutexGuard<'a, T>, tokio::sync::TryLockError> {
        let guard = self.mutex.try_lock();
        trace!(
            "try-async-locking {} for {} ({})",
            self.name,
            reason,
            if guard.is_ok() { "success" } else { "failure" }
        );
        guard.map(|guard| AsyncMutexGuard {
            name: self.name,
            reason,
            guard,
        })
    }

    pub async fn lock<'a>(&'a self, reason: &'static str) -> AsyncMutexGuard<'a, T> {
        trace!("async-locking {} for {}", self.name, reason);
        AsyncMutexGuard {
            name: self.name,
            reason,
            guard: self.mutex.lock().await,
        }
    }
}

impl<T: ?Sized> std::ops::Deref for AsyncMutexGuard<'_, T> {
    type Target = T;

    fn deref(&self) -> &T {
        self.guard.deref()
    }
}

impl<T: ?Sized> std::ops::DerefMut for AsyncMutexGuard<'_, T> {
    fn deref_mut(&mut self) -> &mut T {
        self.guard.deref_mut()
    }
}

impl<'a, T: ?Sized> Drop for AsyncMutexGuard<'a, T> {
    fn drop(&mut self) {
        trace!("async-unlocking {} for {}", self.name, self.reason);
    }
}
