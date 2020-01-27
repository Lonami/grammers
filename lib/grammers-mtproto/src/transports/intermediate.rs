use crate::transports::{LengthTooLong, Transport};
use std::io::{Error, ErrorKind, Read, Result, Write};

/// A light MTProto transport protocol available that guarantees data padded
/// to 4 bytes. This is an implementation of the [intermediate transport].
///
/// [intermediate transport]: https://core.telegram.org/mtproto/mtproto-transports#intermediate
pub struct TransportIntermediate;

impl TransportIntermediate {
    /// Creates a new instance of a `TransportIntermediate`.
    pub fn new() -> Self {
        Self
    }
}

impl Transport for TransportIntermediate {
    fn send<W: Write>(&mut self, channel: &mut W, payload: &[u8]) -> Result<()> {
        // payload len + length itself (4 bytes) + send counter (4 bytes) + crc32 (4 bytes)
        let len = payload.len() as u32;
        channel.write_all(&len.to_le_bytes())?;
        channel.write_all(payload)?;
        Ok(())
    }

    fn receive<R: Read>(&mut self, channel: &mut R, buffer: &mut Vec<u8>) -> Result<()> {
        let len = {
            let mut buf = [0; 4];
            channel.read_exact(&mut buf)?;
            u32::from_le_bytes(buf)
        };

        if len > Self::MAXIMUM_DATA {
            return Err(Error::new(ErrorKind::InvalidInput, LengthTooLong { len }));
        }

        buffer.resize((len - 12) as usize, 0);
        channel.read_exact(buffer)?;

        Ok(())
    }
}
