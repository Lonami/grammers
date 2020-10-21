// Copyright 2020 - developers of the `grammers` project.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// https://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or https://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

//! Implementation of the several [MTProto transports]. This layer is
//! responsible for taking serialized messages from the MTP and packing them
//! in a format that can be sent over a protocol, such as TCP, HTTP or UDP.
//!
//! [MTProto transports]: https://core.telegram.org/mtproto#mtproto-transport
mod abridged;
mod full;
//mod intermediate;

use crate::errors::TransportError;
pub use abridged::Abridged;
pub use full::Full;
//pub use intermediate::Intermediate;

/// The trait used by the transports to create instances of themselves.
pub trait Transport {
    /// Packs and writes `input` into `output`.
    ///
    /// Previous contents in `output` are not cleared before this operation.
    ///
    /// Panics if `input.len()` is not divisible by 4.
    fn pack(&mut self, input: &[u8], output: &mut Vec<u8>);

    /// Unpacks the content from `input` into `output`.
    ///
    /// Previous contents in `output` are not cleared before this operation.
    fn unpack(&mut self, input: &[u8], output: &mut Vec<u8>) -> Result<(), TransportError>;
}
