// Copyright 2020 - developers of the `grammers` project.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// https://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or https://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

use crate::Client;
pub use grammers_mtsender::InvocationError;
use std::collections::VecDeque;

/// Common parts to all requests that are used for creating iterators.
///
/// End-users should obtain particular instances of this type via client methods.
pub struct IterBuffer<R, T> {
    pub(crate) client: Client,
    pub(crate) limit: Option<usize>,
    pub(crate) fetched: usize,
    pub(crate) buffer: VecDeque<T>,
    pub(crate) last_chunk: bool,
    pub(crate) total: Option<usize>,
    pub(crate) request: R,
}

impl<R, T> IterBuffer<R, T> {
    /// Create a new `IterBuffer` instance from a handle, capacity and request.
    pub(crate) fn from_request(client: &Client, capacity: usize, request: R) -> Self {
        Self {
            client: client.clone(),
            limit: None,
            fetched: 0,
            buffer: VecDeque::with_capacity(capacity),
            last_chunk: false,
            total: None,
            request,
        }
    }

    /// Change how many items will be returned from the iterator.
    ///
    /// Using `limit` instead of `take` on the iterator is useful because outgoing requests can
    /// ask for less items from the server to only fetch what's needed.
    pub fn limit(mut self, n: usize) -> Self {
        self.limit = Some(n);
        self
    }

    /// Checks whether the limit has been reached and no more items should be fetched.
    fn limit_reached(&self) -> bool {
        if let Some(limit) = self.limit {
            self.fetched >= limit
        } else {
            false
        }
    }

    /// Return the next result item from the buffer unless more data needs to be fetched.
    ///
    /// Data does not need to be fetched if the limit is reached or the buffer is empty and the
    /// last chunk was reached.
    pub(crate) fn next_raw(&mut self) -> Option<Result<Option<T>, InvocationError>> {
        if self.limit_reached() || (self.buffer.is_empty() && self.last_chunk) {
            Some(Ok(None))
        } else {
            self.pop_item().map(|item| Ok(Some(item)))
        }
    }

    /// Determines the new "limit" for the request, so that no unnecessary items are fetched from
    /// the network.
    pub(crate) fn determine_limit(&self, max: usize) -> i32 {
        if let Some(limit) = self.limit {
            if self.fetched < limit {
                (limit - self.fetched).min(max) as i32
            } else {
                1 // 0 would cause Telegram to send a default amount and not actually 0
            }
        } else {
            max as i32
        }
    }

    /// Pop a buffered item from the queue, and increment the amount of items fetched (returned).
    pub(crate) fn pop_item(&mut self) -> Option<T> {
        if let Some(item) = self.buffer.pop_front() {
            self.fetched += 1;
            Some(item)
        } else {
            None
        }
    }
}
