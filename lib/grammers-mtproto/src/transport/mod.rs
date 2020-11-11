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
pub use intermediate::Intermediate;
use std::fmt;

use bytes::BytesMut;

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
    BadLen { got: u32 },

    /// The sequence number received does not match the expected value.
    BadSeq { expected: u32, got: u32 },

    /// The checksum of the packet does not match its expected value.
    BadCrc { expected: u32, got: u32 },
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
    /// Packs and writes `input` into `output`.
    ///
    /// Previous contents in `output` are not cleared before this operation.
    ///
    /// Panics if `input.len()` is not divisible by 4.
    fn pack(&mut self, input: &[u8], output: &mut BytesMut);

    /// Unpacks the content from `input` into `output`.
    ///
    /// Previous contents in `output` are not cleared before this operation.
    ///
    /// If successful, returns how many bytes of `input` were used.
    fn unpack(&mut self, input: &[u8], output: &mut BytesMut) -> Result<usize, Error>;
}
