// Copyright 2020 - developers of the `grammers` project.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// https://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or https://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.
use grammers_tl_types::RemoteCall;

/// Most requests can fetch 100 messages at once.
const MAX_ITEMS_PER_REQUEST: usize = 100;

/// A generic helper to aid implementing `RPCIterator`. It buffers the items
/// to be yield and also stores some common parts to all iterators, such as
/// being done, the total, and the request they need.
pub(crate) struct RpcIterBuffer<R: RemoteCall, T> {
    pub batch: Vec<T>,
    pub done: bool,
    pub total: Option<usize>,
    pub request: R,
}

impl<R: RemoteCall, T> RpcIterBuffer<R, T> {
    pub fn new(request: R) -> Self {
        RpcIterBuffer {
            batch: Vec::with_capacity(MAX_ITEMS_PER_REQUEST),
            done: false,
            total: None,
            request,
        }
    }

    #[inline(always)]
    pub fn should_fill(&self) -> bool {
        self.batch.is_empty() && !self.done
    }

    #[inline(always)]
    pub fn pop(&mut self) -> Option<T> {
        self.batch.pop()
    }

    #[inline(always)]
    pub fn push(&mut self, value: T) {
        self.batch.push(value);
    }
}
