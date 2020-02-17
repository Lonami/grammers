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
mod abridged;
mod full;
mod intermediate;

pub use abridged::TransportAbridged;
pub use full::TransportFull;
pub use intermediate::TransportIntermediate;

use std::error::Error;
use std::fmt;

/// This error occurs when the data to be read is too long.
#[derive(Debug)]
pub struct LengthTooLong {
    pub len: u32,
}

impl Error for LengthTooLong {}

impl fmt::Display for LengthTooLong {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "incoming packet length is too long: {:08x}", self.len)
    }
}

/// This error occurs when the received data's checksum does not match.
#[derive(Debug)]
pub struct InvalidCrc32 {
    pub got: u32,
    pub expected: u32,
}

impl Error for InvalidCrc32 {}

impl fmt::Display for InvalidCrc32 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "incoming packet's crc32 does not match: got {:08x}, expected {:08x}",
            self.got, self.expected
        )
    }
}

/// The trait used by [MTProto transports].
///
/// [MTProto transports]: index.html
pub trait Transport: Default {
    /// The maximum data that can be received in a single packet.
    /// Anything bigger than this will result in an error to avoid attacks.
    const MAXIMUM_DATA: u32 = 2 * 1024 * 1024;

    /// How much overhead does the transport incur, at a maximum.
    // TODO review naming inconsistencies
    const MAX_OVERHEAD: usize;

    // TODO consider more specific types
    /// Write the packet from `input` into `output`.
    ///
    /// On success, return how many bytes were written.
    ///
    /// On failure, return how many bytes long the output buffer should have been.
    fn write_into<'a>(&mut self, input: &[u8], output: &mut [u8]) -> Result<usize, usize>;

    /// Read a packet from `input` and return the body subslice.
    ///
    /// On success, return how many bytes were written.
    ///
    /// On failure, return how many bytes long the input buffer should have been.
    fn read<'a>(&mut self, input: &'a [u8]) -> Result<&'a [u8], usize>;
}
