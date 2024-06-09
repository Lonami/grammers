// Copyright 2020 - developers of the `grammers` project.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// https://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or https://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.
use super::{Error, Transport, UnpackedOffset};
use grammers_crypto::RingBuffer;

/// The lightest MTProto transport protocol available. This is an
/// implementation of the [abridged transport].
///
/// * Overhead: very small.
/// * Minimum envelope length: 1 byte.
/// * Maximum envelope length: 4 bytes.
///
/// It serializes the input payload as follows, if the length is small enough:
///
/// ```text
/// +-+----...----+
/// |L|  payload  |
/// +-+----...----+
///  ^ 1 byte
/// ```
///
/// Otherwise:
///
/// ```text
/// +----+----...----+
/// | len|  payload  |
/// +----+----...----+
///  ^^^^ 4 bytes
/// ```
///
/// [abridged transport]: https://core.telegram.org/mtproto/mtproto-transports#abridged
pub struct Abridged {
    init: bool,
}

#[allow(clippy::new_without_default)]
impl Abridged {
    pub fn new() -> Self {
        Self { init: false }
    }
}

impl Transport for Abridged {
    fn pack(&mut self, buffer: &mut RingBuffer<u8>) {
        let len = buffer.len();
        assert_eq!(len % 4, 0);

        let len = len / 4;
        if len < 127 {
            buffer.shift(1).extend([len as u8]);
        } else {
            buffer
                .shift(4)
                .extend((0x7f | ((len as u32) << 8)).to_le_bytes());
        }

        if !self.init {
            buffer.shift(1).extend([0xef]);
            self.init = true;
        }
    }

    fn unpack(&mut self, buffer: &[u8]) -> Result<UnpackedOffset, Error> {
        if buffer.is_empty() {
            return Err(Error::MissingBytes);
        }

        let header_len;
        let len = buffer[0];
        let len = if len < 127 {
            header_len = 1;
            len as i32
        } else {
            if buffer.len() < 4 {
                return Err(Error::MissingBytes);
            }

            header_len = 4;
            i32::from_le_bytes(buffer[0..4].try_into().unwrap()) >> 8
        };

        let len = len * 4;
        if (buffer.len() as i32) < header_len + len {
            return Err(Error::MissingBytes);
        }

        if header_len == 1 && len >= 4 {
            let data = i32::from_le_bytes(buffer[1..5].try_into().unwrap());
            if data < 0 {
                return Err(Error::BadStatus {
                    status: (-data) as u32,
                });
            }
        }

        let header_len = header_len as usize;
        let len = len as usize;

        Ok(UnpackedOffset {
            data_start: header_len,
            data_end: header_len + len,
            next_offset: header_len + len,
        })
    }

    fn reset(&mut self) {
        self.init = false;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Returns a new abridged transport, and `n` bytes of input data for it.
    fn setup_pack(n: usize) -> (Abridged, RingBuffer<u8>) {
        let mut buffer = RingBuffer::with_capacity(n, 0);
        buffer.extend((0..n).map(|x| (x & 0xff) as u8));
        (Abridged::new(), buffer)
    }

    #[test]
    fn pack_empty() {
        let (mut transport, mut buffer) = setup_pack(0);
        transport.pack(&mut buffer);
        assert_eq!(&buffer[..], &[0xef, 0]);
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
        assert_eq!(&buffer[..2], &[0xef, 32]);
        assert_eq!(&buffer[2..], &orig[..]);
    }

    #[test]
    fn pack_large() {
        let (mut transport, mut buffer) = setup_pack(1024);
        let orig = buffer.clone();
        transport.pack(&mut buffer);
        assert_eq!(&buffer[..5], &[0xef, 127, 0, 1, 0]);
        assert_eq!(&buffer[5..], &orig[..]);
    }

    #[test]
    fn unpack_small() {
        let mut transport = Abridged::new();
        let mut buffer = RingBuffer::with_capacity(1, 0);
        buffer.extend([1]);
        assert_eq!(transport.unpack(&buffer[..]), Err(Error::MissingBytes));
    }

    #[test]
    fn unpack_normal() {
        let (mut transport, mut buffer) = setup_pack(128);
        let orig = buffer.clone();
        transport.pack(&mut buffer);
        buffer.skip(1); // init byte
        let offset = transport.unpack(&buffer[..]).unwrap();
        assert_eq!(&buffer[offset.data_start..offset.data_end], &orig[..]);
    }

    #[test]
    fn unpack_two_at_once() {
        let (mut transport, mut buffer) = setup_pack(128);
        let orig = buffer.clone();

        let mut two_buffer = RingBuffer::with_capacity(0, 0);
        transport.pack(&mut buffer);
        two_buffer.extend(&buffer[1..]); // init byte
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
    fn unpack_large() {
        let (mut transport, mut buffer) = setup_pack(1024);
        let orig = buffer.clone();
        transport.pack(&mut buffer);
        buffer.skip(1); // init byte
        let offset = transport.unpack(&buffer[..]).unwrap();
        assert_eq!(&buffer[offset.data_start..offset.data_end], &orig[..]);
    }

    #[test]
    fn unpack_bad_status() {
        let mut transport = Abridged::new();
        let mut buffer = RingBuffer::with_capacity(5, 0);
        buffer.push(1u8);
        buffer.extend(&(-404_i32).to_le_bytes());

        assert_eq!(
            transport.unpack(&buffer[..]),
            Err(Error::BadStatus { status: 404 })
        );
    }
}
