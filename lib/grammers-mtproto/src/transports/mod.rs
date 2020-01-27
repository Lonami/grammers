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
use std::io::{Read, Result, Write};

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

/// Anything implementing this trait can be used as a transport.
pub trait Transport {
    /// The maximum data that can be received in a single packet.
    /// Anything bigger than this will result in an error to avoid attacks.
    const MAXIMUM_DATA: u32 = 2 * 1024 * 1024;

    /// Send a packet.
    fn send<W: Write>(&mut self, channel: &mut W, payload: &[u8]) -> Result<()>;

    /// Receive a packet into an existing buffer.
    fn receive_into<R: Read>(&mut self, channel: &mut R, buffer: &mut Vec<u8>) -> Result<()>;

    /// Create a new buffer to hold an incoming message,
    /// and then receive one into it.
    fn receive<R: Read>(&mut self, channel: &mut R) -> Result<Vec<u8>> {
        let mut buffer = Vec::new();
        match self.receive_into(channel, &mut buffer) {
            Ok(_) => Ok(buffer),
            Err(e) => Err(e),
        }
    }
}
