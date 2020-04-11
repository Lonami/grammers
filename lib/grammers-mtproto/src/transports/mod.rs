// Copyright 2020 - developers of the `grammers` project.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// https://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or https://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

//! Implementation of the several [MTProto transports].
//!
//! [MTProto transports]: https://core.telegram.org/mtproto#mtproto-transport
//mod abridged;
mod full;
//mod intermediate;

use crate::errors::TransportError;

//pub use abridged::TransportAbridged;
pub use full::full_transport;
//pub use intermediate::TransportIntermediate;

/// The trait used by [MTProto transports]' encoders.
///
/// [MTProto transports]: index.html
pub trait Encoder {
    /// How much overhead does the transport incur, at a maximum.
    fn max_overhead(&self) -> usize;

    /// Write the packet from `input` into `output`.
    ///
    /// On success, return how many bytes were written.
    ///
    /// On failure, return how many bytes long the output buffer should have been.
    fn write_into<'a>(&mut self, input: &[u8], output: &mut [u8]) -> Result<usize, usize>;
}

/// The trait used by [MTProto transports]' decoders.
///
/// [MTProto transports]: index.html
pub trait Decoder {
    /// Read a packet from `input` and return the body subslice.
    ///
    /// On success, return how many bytes were written.
    ///
    /// On failure, return either how many bytes long the input buffer should
    /// have been or decoding failure in which case the connection should end.
    fn read<'a>(&mut self, input: &'a [u8]) -> Result<&'a [u8], TransportError>;
}
