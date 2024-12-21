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
mod obfuscated;

pub use abridged::Abridged;
pub use full::Full;
use grammers_crypto::DequeBuffer;
pub use intermediate::Intermediate;
pub use obfuscated::Obfuscated;
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

    /// A negative length was received, indicating a [transport-level error].
    /// The absolute value of this length behaves like an [HTTP status code]:
    ///
    /// * 404, if the authorization key used was not found, meaning that the
    ///   server is not aware of the key used by the client, so it cannot be
    ///   used to securely communicate with it.
    ///
    /// * 429, if too many transport connections are established to the same
    ///   IP address in a too-short lapse of time.
    ///
    /// [transport-level error]: https://core.telegram.org/mtproto/mtproto-transports#transport-errors
    /// [HTTP status code]: https://developer.mozilla.org/en-US/docs/Web/HTTP/Status
    BadStatus { status: u32 },
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
            Error::BadLen { got } => write!(f, "bad len (got {got})"),
            Error::BadSeq { expected, got } => {
                write!(f, "bad seq (expected {expected}, got {got})")
            }
            Error::BadCrc { expected, got } => {
                write!(f, "bad crc (expected {expected}, got {got})")
            }
            Error::BadStatus { status } => {
                write!(f, "bad status (negative length -{status})")
            }
        }
    }
}

/// The trait used by the transports to create instances of themselves.
pub trait Transport {
    /// Packs the input buffer in-place.
    ///
    /// Panics if `input.len()` is not divisible by 4.
    fn pack(&mut self, buffer: &mut DequeBuffer<u8>);

    /// Unpacks the input buffer in-place.
    /// Subsequent calls to `unpack` should be made with the same buffer,
    /// with the data on the ranges from previous `UnpackedOffset` removed.
    fn unpack(&mut self, buffer: &mut [u8]) -> Result<UnpackedOffset, Error>;

    /// Reset the state, as if a new instance was just created.
    fn reset(&mut self);
}

/// The trait used by the obfuscated transport to get the transport tags.
pub trait Tagged {
    /// Gets the transport tag for use in the obfuscated transport and
    /// changes the internal state to avoid sending the tag again.
    fn init_tag(&mut self) -> [u8; 4];
}
