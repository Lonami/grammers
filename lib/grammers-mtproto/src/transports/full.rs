// Copyright 2020 - developers of the `grammers` project.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// https://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or https://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.
use crate::errors::TransportError;
use crate::transports::{Decoder, Encoder, Transport};
use crc::crc32::{self, Hasher32};

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
pub struct TransportFull;

impl Transport for TransportFull {
    type Encoder = FullEncoder;
    type Decoder = FullDecoder;

    fn instance() -> (Self::Encoder, Self::Decoder) {
        (Self::Encoder { counter: 0 }, Self::Decoder { counter: 0 })
    }
}

pub struct FullEncoder {
    counter: u32,
}

pub struct FullDecoder {
    counter: u32,
}

impl Encoder for FullEncoder {
    fn max_overhead(&self) -> usize {
        12
    }

    fn write_magic(&mut self, _output: &mut [u8]) -> Result<usize, usize> {
        Ok(0)
    }

    fn write_into<'a>(&mut self, input: &[u8], output: &mut [u8]) -> Result<usize, usize> {
        assert_eq!(input.len() % 4, 0);

        // payload len + length itself (4 bytes) + send counter (4 bytes) + crc32 (4 bytes)
        let len = input.len() + 4 + 4 + 4;
        if output.len() < len {
            return Err(len);
        }

        let len_bytes = (len as u32).to_le_bytes();
        let counter = self.counter.to_le_bytes();

        let crc = {
            let mut digest = crc32::Digest::new(crc32::IEEE);
            digest.write(&len_bytes);
            digest.write(&counter);
            digest.write(input);
            digest.sum32().to_le_bytes()
        };

        // We could use `io::Cursor`, and even though we know `write_all`
        // would never fail (we checked `output.len()` above), we would
        // still need to add several `unwrap()`. The only benefit would
        // be not keeping track of the offsets manually. Not worth it.
        output[0..4].copy_from_slice(&len_bytes);
        output[4..8].copy_from_slice(&counter);
        output[8..len - 4].copy_from_slice(input);
        output[len - 4..len].copy_from_slice(&crc);

        self.counter += 1;
        Ok(len)
    }
}

impl Decoder for FullDecoder {
    fn read<'a>(&mut self, input: &'a [u8]) -> Result<&'a [u8], TransportError> {
        // TODO the input and output len can probably be abstracted away
        //      ("minimal input" and "calculate output len")
        // Need 4 bytes for the initial length
        if input.len() < 4 {
            return Err(TransportError::MissingBytes(4));
        }

        // payload len
        let mut len_data = [0; 4];
        len_data.copy_from_slice(&input[0..4]);
        let len = u32::from_le_bytes(len_data) as usize;
        if input.len() < len {
            return Err(TransportError::MissingBytes(len));
        }

        // receive counter
        let mut counter_data = [0; 4];
        counter_data.copy_from_slice(&input[4..8]);
        let counter = u32::from_le_bytes(counter_data);
        if counter != self.counter {
            return Err(TransportError::UnexpectedData("seq"));
        }

        // payload
        let output = &input[8..len - 4];

        // crc32
        let crc = {
            let mut buf = [0; 4];
            buf.copy_from_slice(&input[len - 4..len]);
            u32::from_le_bytes(buf)
        };

        let valid_crc = {
            let mut digest = crc32::Digest::new(crc32::IEEE);
            digest.write(&len_data);
            digest.write(&counter_data);
            digest.write(output);
            digest.sum32()
        };
        if crc != valid_crc {
            return Err(TransportError::UnexpectedData("crc"));
        }

        self.counter += 1;
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
        let (mut encoder, _) = full_transport();
        let mut output = [];
        assert_eq!(encoder.write_magic(&mut output), Ok(0));
    }

    #[test]
    #[should_panic]
    fn check_non_padded_encoding() {
        let (mut encoder, _) = full_transport();
        let input = get_data(7);
        let mut output = vec![0; 7 + encoder.max_overhead()];
        drop(encoder.write_into(&input, &mut output));
    }

    #[test]
    fn check_encoding() {
        let (mut encoder, _) = full_transport();
        let input = get_data(128);
        let mut output = vec![0; 128 + encoder.max_overhead()];
        assert_eq!(encoder.write_into(&input, &mut output), Ok(140));

        assert_eq!(&output[..4], &[140, 0, 0, 0]);
        assert_eq!(&output[4..8], &[0, 0, 0, 0]);
        assert_eq!(&output[8..8 + input.len()], &input[..]);
        assert_eq!(&output[8 + input.len()..], &[134, 115, 149, 55]);
    }

    #[test]
    fn check_repeated_encoding() {
        let (mut encoder, _) = full_transport();
        let input = get_data(128);
        let mut output = vec![0; 128 + encoder.max_overhead()];
        assert!(encoder.write_into(&input, &mut output).is_ok());
        assert!(encoder.write_into(&input, &mut output).is_ok());

        assert_eq!(&output[..4], &[140, 0, 0, 0]);
        assert_eq!(&output[4..8], &[1, 0, 0, 0]);
        assert_eq!(&output[8..8 + input.len()], &input[..]);
        assert_eq!(&output[8 + input.len()..], &[150, 9, 240, 74]);
    }

    #[test]
    fn check_encoding_small_buffer() {
        let (mut encoder, _) = full_transport();
        let input = get_data(128);
        let mut output = vec![0; 8];
        assert_eq!(encoder.write_into(&input, &mut output), Err(140));
    }

    #[test]
    fn check_decoding() {
        let (mut encoder, mut decoder) = full_transport();
        let input = get_data(128);
        let mut output = vec![0; 128 + encoder.max_overhead()];
        encoder.write_into(&input, &mut output).unwrap();
        assert_eq!(decoder.read(&output), Ok(&input[..]));
    }

    #[test]
    fn check_repeating_decoding() {
        let (mut encoder, mut decoder) = full_transport();
        let input = get_data(128);
        let mut output = vec![0; 128 + encoder.max_overhead()];

        encoder.write_into(&input, &mut output).unwrap();
        assert_eq!(decoder.read(&output), Ok(&input[..]));
        encoder.write_into(&input, &mut output).unwrap();
        assert_eq!(decoder.read(&output), Ok(&input[..]));
    }

    #[test]
    fn check_bad_crc_decoding() {
        let (mut encoder, mut decoder) = full_transport();
        let input = get_data(128);
        let mut output = vec![0; 128 + encoder.max_overhead()];

        encoder.write_into(&input, &mut output).unwrap();
        let out_len = output.len() - 1;
        output[out_len] ^= 0xff;
        assert_eq!(
            decoder.read(&output),
            Err(TransportError::UnexpectedData("crc"))
        );
    }

    #[test]
    fn check_bad_repeating_decoding() {
        let (mut encoder, mut decoder) = full_transport();
        let input = get_data(128);
        let mut output = vec![0; 128 + encoder.max_overhead()];

        encoder.write_into(&input, &mut output).unwrap();
        assert_eq!(decoder.read(&output), Ok(&input[..]));
        assert_eq!(
            decoder.read(&output),
            Err(TransportError::UnexpectedData("seq"))
        );
    }

    #[test]
    fn check_decoding_small_buffer() {
        let (_, mut decoder) = full_transport();
        let input = get_data(3);
        assert_eq!(decoder.read(&input), Err(TransportError::MissingBytes(4)));
    }
}
