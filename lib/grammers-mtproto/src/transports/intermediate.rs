// Copyright 2020 - developers of the `grammers` project.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// https://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or https://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.
use crate::errors::TransportError;
use crate::transports::{Decoder, Encoder};

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
pub fn intermediate_transport() -> (IntermediateEncoder, IntermediateDecoder) {
    (IntermediateEncoder, IntermediateDecoder)
}

#[non_exhaustive]
pub struct IntermediateEncoder;

#[non_exhaustive]
pub struct IntermediateDecoder;

impl Encoder for IntermediateEncoder {
    fn max_overhead(&self) -> usize {
        4
    }

    fn write_magic(&mut self, output: &mut [u8]) -> Result<usize, usize> {
        if output.len() < 4 {
            Err(4)
        } else {
            output[..4].copy_from_slice(&[0xee, 0xee, 0xee, 0xee]);
            Ok(4)
        }
    }

    fn write_into<'a>(&mut self, input: &[u8], output: &mut [u8]) -> Result<usize, usize> {
        // payload len + length itself (4 bytes) + send counter (4 bytes) + crc32 (4 bytes)
        let len = input.len() + 4;
        if output.len() < len {
            return Err(len);
        }

        output[0..4].copy_from_slice(&(input.len() as u32).to_le_bytes());
        output[4..len].copy_from_slice(input);
        Ok(len)
    }
}

impl Decoder for IntermediateDecoder {
    fn read<'a>(&mut self, input: &'a [u8]) -> Result<&'a [u8], TransportError> {
        if input.len() < 4 {
            return Err(TransportError::MissingBytes(4));
        }

        let len = {
            let mut buf = [0; 4];
            buf.copy_from_slice(&input[0..4]);
            u32::from_le_bytes(buf)
        } as usize
            + 4;

        if input.len() < len {
            return Err(TransportError::MissingBytes(len));
        }

        let output = &input[4..len - 4];
        Ok(output)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn get_data(n: usize) -> Vec<u8> {
        let mut result = Vec::with_capacity(n);
        for i in 0..n {
            result.push((i & 0xff) as u8);
        }
        result
    }

    #[test]
    fn check_magic() {
        let (mut encoder, _) = intermediate_transport();
        let mut output = [0, 0, 0, 0];
        assert_eq!(encoder.write_magic(&mut output), Ok(4));
        assert_eq!(output, [0xee, 0xee, 0xee, 0xee]);
    }

    #[test]
    fn check_encoding() {
        let (mut encoder, _) = intermediate_transport();
        let input = get_data(128);
        let mut output = vec![0; 128 + encoder.max_overhead()];
        assert_eq!(encoder.write_into(&input, &mut output), Ok(132));

        assert_eq!(&output[..4], &[128, 0, 0, 0]);
        assert_eq!(&output[4..], &input[..]);
    }

    #[test]
    fn check_encoding_small_buffer() {
        let (mut encoder, _) = intermediate_transport();
        let input = get_data(128);
        let mut output = vec![0; 8];
        assert_eq!(encoder.write_into(&input, &mut output), Err(132));
    }

    #[test]
    fn check_decoding() {
        let (mut encoder, mut decoder) = intermediate_transport();
        let input = get_data(128);
        let mut output = vec![0; 128 + encoder.max_overhead()];
        encoder.write_into(&input, &mut output).unwrap();
        assert_eq!(decoder.read(&output), Ok(&input[..]));
    }

    #[test]
    fn check_decoding_small_buffer() {
        let (_, mut decoder) = intermediate_transport();
        let input = get_data(3);
        assert_eq!(decoder.read(&input), Err(TransportError::MissingBytes(4)));
    }
}
