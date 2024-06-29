// Copyright 2020 - developers of the `grammers` project.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// https://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or https://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.
use crate::mtp::DeserializeError;

/// Checks a message buffer for common errors
pub(crate) fn check_message_buffer(message: &[u8]) -> Result<(), DeserializeError> {
    if message.len() < 20 {
        Err(DeserializeError::MessageBufferTooSmall)
    } else {
        Ok(())
    }
}

/// Stack buffer to support extending from an iterator.
pub(crate) struct StackBuffer<const N: usize> {
    array: [u8; N],
    pos: usize,
}

impl<const N: usize> StackBuffer<N> {
    pub(crate) fn new() -> Self {
        Self {
            array: [0; N],
            pos: 0,
        }
    }

    pub(crate) fn into_inner(self) -> [u8; N] {
        self.array
    }
}

impl<const N: usize> Extend<u8> for StackBuffer<N> {
    fn extend<T: IntoIterator<Item = u8>>(&mut self, iter: T) {
        iter.into_iter().for_each(|x| {
            self.array[self.pos] = x;
            self.pos += 1;
        });
    }
}
