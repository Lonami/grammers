use crate::transports::{InvalidCrc32, LengthTooLong, Transport};
use crc::crc32::{self, Hasher32};
use std::io::{Error, ErrorKind, Read, Result, Write};

/// The basic MTProto transport protocol. This is an implementation of the
/// [full transport].
///
/// * Overhead: medium
/// * Minimum envelope length: 12 bytes.
/// * Maximum envelope length: 12 bytes.
///
/// [full transport]: https://core.telegram.org/mtproto/mtproto-transports#full
pub struct TransportFull {
    send_counter: u32,
}

impl TransportFull {
    /// Creates a new instance of a `TransportFull`.
    pub fn new() -> Self {
        Self { send_counter: 0 }
    }
}

impl Transport for TransportFull {
    fn send<W: Write>(&mut self, channel: &mut W, payload: &[u8]) -> Result<()> {
        // payload len + length itself (4 bytes) + send counter (4 bytes) + crc32 (4 bytes)
        let len = (payload.len() + 4 + 4 + 4) as u32;
        let len = len.to_le_bytes();
        let counter = self.send_counter.to_le_bytes();

        let crc = {
            let mut digest = crc32::Digest::new(crc32::IEEE);
            digest.write(&len);
            digest.write(&counter);
            digest.write(payload);
            digest.sum32().to_le_bytes()
        };

        channel.write_all(&len)?;
        channel.write_all(&counter)?;
        channel.write_all(payload)?;
        channel.write_all(&crc)?;
        self.send_counter += 1;
        Ok(())
    }

    fn receive_into<R: Read>(&mut self, channel: &mut R, buffer: &mut Vec<u8>) -> Result<()> {
        // payload len
        let mut len_data = [0; 4];
        channel.read_exact(&mut len_data)?;
        let len = u32::from_le_bytes(len_data);
        if len > Self::MAXIMUM_DATA {
            return Err(Error::new(ErrorKind::InvalidInput, LengthTooLong { len }));
        }

        // receive counter
        let mut counter_data = [0; 4];
        channel.read_exact(&mut counter_data)?;
        let _counter = u32::from_le_bytes(counter_data);

        // payload
        buffer.resize((len - 12) as usize, 0);
        channel.read_exact(buffer)?;

        // crc32
        let crc = {
            let mut buf = [0; 4];
            channel.read_exact(&mut buf)?;
            u32::from_le_bytes(buf)
        };

        let valid_crc = {
            let mut digest = crc32::Digest::new(crc32::IEEE);
            digest.write(&len_data);
            digest.write(&counter_data);
            digest.write(buffer);
            digest.sum32()
        };
        if crc != valid_crc {
            return Err(Error::new(
                ErrorKind::InvalidInput,
                InvalidCrc32 {
                    got: crc,
                    expected: valid_crc,
                },
            ));
        }

        Ok(())
    }
}
