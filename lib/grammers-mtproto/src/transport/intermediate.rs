// Copyright 2020 - developers of the `grammers` project.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// https://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or https://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.
use super::{Error, Transport, UnpackedOffset};
use grammers_crypto::RingBuffer;

/// A light MTProto transport protocol available that guarantees data padded
/// to 4 bytes. This is an implementation of the [intermediate transport].
///
/// * Overhead: small.
/// * Minimum envelope length: 4 bytes.
/// * Maximum envelope length: 4 bytes.
///
/// It serializes the input payload as follows:
///
/// ```text
/// +----+----...----+
/// | len|  payload  |
/// +----+----...----+
///  ^^^^ 4 bytes
/// ```
///
/// [intermediate transport]: https://core.telegram.org/mtproto/mtproto-transports#intermediate
pub struct Intermediate {
    init: bool,
}

#[allow(clippy::new_without_default)]
impl Intermediate {
    pub fn new() -> Self {
        Self { init: false }
    }
}

impl Transport for Intermediate {
    fn pack(&mut self, buffer: &mut RingBuffer<u8>) {
        let len = buffer.len();
        assert_eq!(len % 4, 0);

        buffer.shift(4).extend((len as i32).to_le_bytes());

        if !self.init {
            buffer.shift(4).extend(0xee_ee_ee_ee_u32.to_le_bytes());
            self.init = true;
        }
    }

    fn unpack(&mut self, buffer: &[u8]) -> Result<UnpackedOffset, Error> {
        if buffer.len() < 4 {
            return Err(Error::MissingBytes);
        }

        let len = i32::from_le_bytes(buffer[0..4].try_into().unwrap());
        if (buffer.len() as i32) < len {
            return Err(Error::MissingBytes);
        }

        if len <= 4 {
            if len >= 4 {
                let data = i32::from_le_bytes(buffer[4..8].try_into().unwrap());
                return Err(Error::BadStatus {
                    status: (-data) as u32,
                });
            }
            return Err(Error::BadLen { got: len });
        }

        let len = len as usize;

        Ok(UnpackedOffset {
            data_start: 4,
            data_end: 4 + len,
            next_offset: 4 + len,
        })
    }

    fn reset(&mut self) {
        self.init = false;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Returns a full intermediate transport, and `n` bytes of input data for it.
    fn setup_pack(n: usize) -> (Intermediate, RingBuffer<u8>) {
        let mut buffer = RingBuffer::with_capacity(n, 0);
        buffer.extend((0..n).map(|x| (x & 0xff) as u8));
        (Intermediate::new(), buffer)
    }

    #[test]
    fn pack_empty() {
        let (mut transport, mut buffer) = setup_pack(0);
        transport.pack(&mut buffer);
        assert_eq!(&buffer[..], &[0xee, 0xee, 0xee, 0xee, 0, 0, 0, 0]);
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
        assert_eq!(&buffer[..8], &[0xee, 0xee, 0xee, 0xee, 128, 0, 0, 0]);
        assert_eq!(&buffer[8..buffer.len()], &orig[..]);
    }

    #[test]
    fn unpack_small() {
        let mut transport = Intermediate::new();
        let mut buffer = RingBuffer::with_capacity(1, 0);
        buffer.extend([1]);
        assert_eq!(transport.unpack(&buffer[..],), Err(Error::MissingBytes));
    }

    #[test]
    fn unpack_normal() {
        let (mut transport, mut buffer) = setup_pack(128);
        let orig = buffer.clone();
        transport.pack(&mut buffer);
        buffer.skip(4); // init bytes
        let offset = transport.unpack(&buffer[..]).unwrap();
        assert_eq!(&buffer[offset.data_start..offset.data_end], &orig[..]);
    }

    #[test]
    fn unpack_two_at_once() {
        let (mut transport, mut buffer) = setup_pack(128);
        let orig = buffer.clone();

        let mut two_buffer = RingBuffer::with_capacity(0, 0);
        transport.pack(&mut buffer);
        two_buffer.extend(&buffer[4..]); // init bytes
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
    fn unpack_bad_status() {
        let mut transport = Intermediate::new();
        let mut buffer = RingBuffer::with_capacity(8, 0);
        buffer.extend(&(4_i32).to_le_bytes());
        buffer.extend(&(-404_i32).to_le_bytes());

        assert_eq!(
            transport.unpack(&buffer[..]),
            Err(Error::BadStatus { status: 404 })
        );
    }
}
