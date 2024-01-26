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
mod intermediate;

pub use abridged::Abridged;
pub use full::Full;
use grammers_crypto::RingBuffer;
pub use intermediate::Intermediate;
use std::fmt;

/// The error type reported by the different transports when something is wrong.
///
/// Certain transports will only produce certain variants of this error.
///
/// Unless the variant is `MissingBytes`, the connection should not continue.
#[derive(Clone, Debug, PartialEq)]
pub enum Error {
    /// Not enough bytes are provided.
    MissingBytes,

    /// The length is either too short or too long to represent a valid packet.
    BadLen { got: i32 },

    /// The sequence number received does not match the expected value.
    BadSeq { expected: i32, got: i32 },

    /// The checksum of the packet does not match its expected value.
    BadCrc { expected: u32, got: u32 },
}

#[derive(Clone, Debug, PartialEq)]
pub struct UnpackedOffset {
    pub data_start: usize,
    pub data_end: usize,
    pub next_offset: usize,
}

impl std::error::Error for Error {}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "transport error: ")?;
        match self {
            Error::MissingBytes => write!(f, "need more bytes"),
            Error::BadLen { got } => write!(f, "bad len (got {})", got),
            Error::BadSeq { expected, got } => {
                write!(f, "bad seq (expected {}, got {})", expected, got)
            }
            Error::BadCrc { expected, got } => {
                write!(f, "bad crc (expected {}, got {})", expected, got)
            }
        }
    }
}

/// The trait used by the transports to create instances of themselves.
pub trait Transport {
    /// Packs the input buffer in-place.
    ///
    /// Panics if `input.len()` is not divisible by 4.
    fn pack(&mut self, buffer: &mut RingBuffer<u8>);

    /// Unpacks the input buffer in-place.
    fn unpack(&mut self, buffer: &[u8]) -> Result<UnpackedOffset, Error>;

    /// Reset the state, as if a new instance was just created.
    fn reset(&mut self);
}
