// Copyright 2020 - developers of the `grammers` project.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// https://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or https://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.
use crate::errors::TransportError;
use crate::transports::{Decoder, Encoder, Transport};

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
pub struct TransportAbridged;

impl Transport for TransportAbridged {
    type Encoder = AbridgedEncoder;
    type Decoder = AbridgedDecoder;

    fn instance() -> (Self::Encoder, Self::Decoder) {
        (Self::Encoder {}, Self::Decoder {})
    }
}

#[non_exhaustive]
pub struct AbridgedEncoder;

#[non_exhaustive]
pub struct AbridgedDecoder;

impl Encoder for AbridgedEncoder {
    fn max_overhead(&self) -> usize {
        4
    }

    fn write_magic(&mut self, output: &mut [u8]) -> Result<usize, usize> {
        if output.len() < 1 {
            Err(1)
        } else {
            output[0] = 0xef;
            Ok(1)
        }
    }

    fn write_into<'a>(&mut self, input: &[u8], output: &mut [u8]) -> Result<usize, usize> {
        assert_eq!(input.len() % 4, 0);

        let len = input.len() / 4;
        let output_len = input.len() + (if len < 127 { 1 } else { 4 });
        if output.len() < output_len {
            return Err(output_len);
        }

        if len < 127 {
            output[0] = len as u8;
            output[1..output_len].copy_from_slice(input);
        } else {
            output[0] = 0x7f;
            output[1..4].copy_from_slice(&len.to_le_bytes()[..3]);
            output[4..output_len].copy_from_slice(input);
        }

        Ok(output_len)
    }
}

impl Decoder for AbridgedDecoder {
    fn read<'a>(&mut self, input: &'a [u8]) -> Result<&'a [u8], TransportError> {
        if input.len() < 1 {
            return Err(TransportError::MissingBytes(1));
        }

        let header_len;
        let len = input[0];
        let len = if len < 127 {
            header_len = 1;
            len as u32
        } else {
            if input.len() < 4 {
                return Err(TransportError::MissingBytes(4));
            }

            header_len = 4;
            let mut len = [0; 4];
            len[..3].copy_from_slice(&input[1..4]);
            u32::from_le_bytes(len)
        };

        let len = len as usize * 4;
        if input.len() < header_len + len {
            return Err(TransportError::MissingBytes(header_len + len));
        }

        Ok(&input[header_len..header_len + len])
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
        let (mut encoder, _) = TransportAbridged::instance();
        let mut output = [0];
        assert_eq!(encoder.write_magic(&mut output), Ok(1));
        assert_eq!(output, [0xef]);
    }

    #[test]
    #[should_panic]
    fn check_non_padded_encoding() {
        let (mut encoder, _) = TransportAbridged::instance();
        let input = get_data(7);
        let mut output = vec![0; 7 + encoder.max_overhead()];
        drop(encoder.write_into(&input, &mut output));
    }

    #[test]
    fn check_encoding() {
        let (mut encoder, _) = TransportAbridged::instance();
        let input = get_data(128);
        let mut output = vec![0; 128 + encoder.max_overhead()];
        let len = encoder.write_into(&input, &mut output).unwrap();
        assert_eq!(&output[..1], &[32]);
        assert_eq!(&output[1..len], &input[..]);
    }

    #[test]
    fn check_large_encoding() {
        let (mut encoder, _) = TransportAbridged::instance();
        let input = get_data(1024);
        let mut output = vec![0; 1024 + encoder.max_overhead()];
        assert!(encoder.write_into(&input, &mut output).is_ok());

        assert_eq!(&output[..4], &[127, 0, 1, 0]);
        assert_eq!(&output[4..], &input[..]);
    }

    #[test]
    fn check_encoding_small_buffer() {
        let (mut encoder, _) = TransportAbridged::instance();
        let input = get_data(128);
        let mut output = vec![0; 64];
        assert_eq!(encoder.write_into(&input, &mut output), Err(129));
    }

    #[test]
    fn check_decoding() {
        let (mut encoder, mut decoder) = TransportAbridged::instance();
        let input = get_data(128);
        let mut output = vec![0; 128 + encoder.max_overhead()];
        encoder.write_into(&input, &mut output).unwrap();
        assert_eq!(decoder.read(&output), Ok(&input[..]));
    }

    #[test]
    fn check_large_decoding() {
        let (mut encoder, mut decoder) = TransportAbridged::instance();
        let input = get_data(1024);
        let mut output = vec![0; 1024 + encoder.max_overhead()];

        encoder.write_into(&input, &mut output).unwrap();
        assert_eq!(decoder.read(&output), Ok(&input[..]));
    }

    #[test]
    fn check_decoding_small_buffer() {
        let (_, mut decoder) = TransportAbridged::instance();
        let input = [1];
        assert_eq!(decoder.read(&input), Err(TransportError::MissingBytes(5)));
    }
}
