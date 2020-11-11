// Copyright 2020 - developers of the `grammers` project.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// https://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or https://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.
use super::{Error, Transport};
use bytes::{Buf, BufMut, BytesMut};

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

impl Intermediate {
    pub fn new() -> Self {
        Self { init: false }
    }
}

impl Transport for Intermediate {
    fn pack(&mut self, input: &[u8], output: &mut BytesMut) {
        assert_eq!(input.len() % 4, 0);

        if !self.init {
            output.put_u32_le(0xee_ee_ee_ee);
            self.init = true;
        }

        output.put_u32_le(input.len() as _);
        output.put(input);
    }

    fn unpack(&mut self, input: &[u8], output: &mut BytesMut) -> Result<usize, Error> {
        if input.len() < 4 {
            return Err(Error::MissingBytes);
        }
        let needle = &mut &input[..];

        let len = needle.get_u32_le() as usize;
        if needle.len() < len {
            return Err(Error::MissingBytes);
        }
        output.put(&needle[..len]);

        Ok(len + 4)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Returns a new abridged transport, `n` bytes of input data for it, and an empty output buffer.
    fn setup_pack(n: u32) -> (Intermediate, Vec<u8>, BytesMut) {
        let input = (0..n).map(|x| (x & 0xff) as u8).collect();
        (Intermediate::new(), input, BytesMut::new())
    }

    #[test]
    fn pack_empty() {
        let (mut transport, input, mut output) = setup_pack(0);
        transport.pack(&input, &mut output);
        assert_eq!(&output[..], &[0xee, 0xee, 0xee, 0xee, 0, 0, 0, 0]);
    }

    #[test]
    #[should_panic]
    fn pack_non_padded() {
        let (mut transport, input, mut output) = setup_pack(7);
        transport.pack(&input, &mut output);
    }

    #[test]
    fn pack_normal() {
        let (mut transport, input, mut output) = setup_pack(128);
        transport.pack(&input, &mut output);
        assert_eq!(&output[..8], &[0xee, 0xee, 0xee, 0xee, 128, 0, 0, 0]);
        assert_eq!(&output[8..output.len()], &input[..]);
    }

    #[test]
    fn unpack_small() {
        let mut transport = Intermediate::new();
        let input = [1];
        let mut output = BytesMut::new();
        assert_eq!(
            transport.unpack(&input, &mut output),
            Err(Error::MissingBytes)
        );
    }

    #[test]
    fn unpack_normal() {
        let (mut transport, input, mut packed) = setup_pack(128);
        let mut unpacked = BytesMut::new();
        transport.pack(&input, &mut packed);
        transport.unpack(&packed[4..], &mut unpacked).unwrap();
        assert_eq!(input, unpacked);
    }
}
