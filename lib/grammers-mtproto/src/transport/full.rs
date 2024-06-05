// Copyright 2020 - developers of the `grammers` project.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// https://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or https://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.
use super::{Error, Transport, UnpackedOffset};
use crc32fast::Hasher;
use grammers_crypto::RingBuffer;

/// The basic MTProto transport protocol. This is an implementation of the
/// [full transport].
///
/// * Overhead: medium
/// * Minimum envelope length: 12 bytes.
/// * Maximum envelope length: 12 bytes.
///
/// It serializes the input payload as follows:
///
/// ```text
/// +----+----+----...----+----+
/// | len| seq|  payload  | crc|
/// +----+----+----...----+----+
///  ^^^^ 4 bytes
/// ```
///
/// [full transport]: https://core.telegram.org/mtproto/mtproto-transports#full
pub struct Full {
    send_seq: i32,
    recv_seq: i32,
}

#[allow(clippy::new_without_default)]
impl Full {
    pub fn new() -> Self {
        Self {
            send_seq: 0,
            recv_seq: 0,
        }
    }
}

impl Transport for Full {
    fn pack(&mut self, buffer: &mut RingBuffer<u8>) {
        let len = buffer.len();
        assert_eq!(len % 4, 0);

        // payload len + length itself (4 bytes) + send counter (4 bytes) + crc32 (4 bytes)
        let len = (len as i32) + 4 + 4 + 4;

        let mut header = buffer.shift(4 + 4);
        header.extend(len.to_le_bytes());
        header.extend(self.send_seq.to_le_bytes());

        let crc = {
            let mut hasher = Hasher::new();
            hasher.update(buffer.as_ref());
            hasher.finalize()
        };
        buffer.extend(crc.to_le_bytes());

        self.send_seq += 1;
    }

    fn unpack(&mut self, buffer: &[u8]) -> Result<UnpackedOffset, Error> {
        // Need 4 bytes for the initial length
        if buffer.len() < 4 {
            return Err(Error::MissingBytes);
        }

        let total_len = buffer.len() as i32;

        // payload len
        let len = i32::from_le_bytes(buffer[0..4].try_into().unwrap());
        if len < 12 {
            return Err(Error::BadLen { got: len });
        }

        if total_len < len {
            return Err(Error::MissingBytes);
        }

        // receive counter
        let seq = i32::from_le_bytes(buffer[4..8].try_into().unwrap());
        if seq != self.recv_seq {
            return Err(Error::BadSeq {
                expected: self.recv_seq,
                got: seq,
            });
        }

        let len = len as usize;

        // crc32
        let crc = u32::from_le_bytes(buffer[len - 4..len].try_into().unwrap());

        let valid_crc = {
            let mut hasher = Hasher::new();
            hasher.update(&buffer[0..len - 4]);
            hasher.finalize()
        };
        if crc != valid_crc {
            return Err(Error::BadCrc {
                expected: valid_crc,
                got: crc,
            });
        }

        self.recv_seq += 1;
        Ok(UnpackedOffset {
            data_start: 8,
            data_end: len - 4,
            next_offset: len,
        })
    }

    fn reset(&mut self) {
        self.recv_seq = 0;
        self.send_seq = 0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Returns a full abridged transport, and `n` bytes of input data for it.
    fn setup_pack(n: usize) -> (Full, RingBuffer<u8>) {
        let mut buffer = RingBuffer::with_capacity(n, 0);
        buffer.extend((0..n).map(|x| (x & 0xff) as u8));
        (Full::new(), buffer)
    }

    #[test]
    fn pack_empty() {
        let (mut transport, mut buffer) = setup_pack(0);
        transport.pack(&mut buffer);

        assert_eq!(&buffer[..], &[12, 0, 0, 0, 0, 0, 0, 0, 38, 202, 141, 50]);
    }

    #[test]
    #[should_panic]
    fn pack_non_padded() {
        let (mut transport, mut buffer) = setup_pack(7);
        transport.pack(&mut buffer);
    }

    #[test]
    fn pack_normal() {
        let (mut transport, mut buffer) = setup_pack(128);
        let orig = buffer.clone();
        transport.pack(&mut buffer);

        assert_eq!(&buffer[..4], &[140, 0, 0, 0]);
        assert_eq!(&buffer[4..8], &[0, 0, 0, 0]);
        assert_eq!(&buffer[8..8 + orig.len()], &orig[..]);
        assert_eq!(&buffer[8 + orig.len()..], &[134, 115, 149, 55]);
    }

    #[test]
    fn pack_twice() {
        let (mut transport, mut buffer) = setup_pack(128);
        let orig = buffer.clone();

        let mut two_buffer = RingBuffer::with_capacity(0, 0);
        transport.pack(&mut buffer);
        two_buffer.extend(&buffer[..]);

        buffer = orig.clone();
        transport.pack(&mut buffer);
        two_buffer.extend(&buffer[..]);

        assert_eq!(&buffer[..4], &[140, 0, 0, 0]);
        assert_eq!(&buffer[4..8], &[1, 0, 0, 0]);
        assert_eq!(&buffer[8..8 + orig.len()], &orig[..]);
        assert_eq!(&buffer[8 + orig.len()..], &[150, 9, 240, 74]);
    }

    #[test]
    fn unpack_small() {
        let mut transport = Full::new();
        let mut buffer = RingBuffer::with_capacity(3, 0);
        buffer.extend([0, 1, 3]);
        assert_eq!(transport.unpack(&buffer[..]), Err(Error::MissingBytes));
    }

    #[test]
    fn unpack_normal() {
        let (mut transport, mut buffer) = setup_pack(128);
        let orig = buffer.clone();
        transport.pack(&mut buffer);
        let offset = transport.unpack(&buffer[..]).unwrap();
        assert_eq!(&buffer[offset.data_start..offset.data_end], &orig[..]);
    }

    #[test]
    fn unpack_two_at_once() {
        let (mut transport, mut buffer) = setup_pack(128);
        let orig = buffer.clone();

        let mut two_buffer = RingBuffer::with_capacity(0, 0);
        transport.pack(&mut buffer);
        two_buffer.extend(&buffer[..]);
        let single_size = two_buffer.len();

        buffer = orig.clone();
        transport.pack(&mut buffer);
        two_buffer.extend(&buffer[..]);

        let offset = transport.unpack(&two_buffer[..]).unwrap();
        assert_eq!(&buffer[offset.data_start..offset.data_end], &orig[..]);
        assert_eq!(offset.next_offset, single_size);

        two_buffer.skip(offset.next_offset);
        let offset = transport.unpack(&two_buffer[..]).unwrap();
        assert_eq!(&buffer[offset.data_start..offset.data_end], &orig[..]);
    }

    #[test]
    fn unpack_bad_seq() {
        let (mut transport, mut buffer) = setup_pack(128);
        transport.pack(&mut buffer);
        buffer[4] = 1;

        assert_eq!(
            transport.unpack(&buffer[..]),
            Err(Error::BadSeq {
                expected: 0,
                got: 1,
            })
        );
    }

    #[test]
    fn unpack_bad_crc() {
        let (mut transport, mut buffer) = setup_pack(128);
        transport.pack(&mut buffer);
        let len = buffer.len();
        buffer[len - 1] ^= 0xff;

        assert_eq!(
            transport.unpack(&buffer[..]),
            Err(Error::BadCrc {
                expected: 932541318,
                got: 3365237638,
            })
        );
    }
}
