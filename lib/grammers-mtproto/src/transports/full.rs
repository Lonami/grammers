// Copyright 2020 - developers of the `grammers` project.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// https://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or https://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.
use crate::errors::TransportError;
use crate::transports::Transport;
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
pub struct Full {
    send_seq: u32,
    recv_seq: u32,
}

impl Full {
    pub fn new() -> Self {
        Self {
            send_seq: 0,
            recv_seq: 0,
        }
    }
}

impl Transport for Full {
    fn pack(&mut self, input: &[u8], output: &mut Vec<u8>) {
        assert_eq!(input.len() % 4, 0);

        // payload len + length itself (4 bytes) + send counter (4 bytes) + crc32 (4 bytes)
        let len = input.len() + 4 + 4 + 4;
        output.reserve(len);

        let len_bytes = (len as u32).to_le_bytes();
        let seq = self.send_seq.to_le_bytes();

        let crc = {
            let mut digest = crc32::Digest::new(crc32::IEEE);
            digest.write(&len_bytes);
            digest.write(&seq);
            digest.write(input);
            digest.sum32().to_le_bytes()
        };

        output.extend_from_slice(&len_bytes);
        output.extend_from_slice(&seq);
        output.extend_from_slice(input);
        output.extend_from_slice(&crc);

        self.send_seq += 1;
    }

    fn unpack(&mut self, input: &[u8], output: &mut Vec<u8>) -> Result<(), TransportError> {
        // Need 4 bytes for the initial length
        if input.len() < 4 {
            return Err(TransportError::MissingBytes(4));
        }

        // payload len
        let mut len_bytes = [0; 4];
        len_bytes.copy_from_slice(&input[0..4]);
        let len = u32::from_le_bytes(len_bytes) as usize;
        if input.len() < len {
            return Err(TransportError::MissingBytes(len));
        }

        // receive counter
        let mut seq_bytes = [0; 4];
        seq_bytes.copy_from_slice(&input[4..8]);
        let seq = u32::from_le_bytes(seq_bytes);
        if seq != self.recv_seq {
            return Err(TransportError::BadSeq {
                expected: self.recv_seq,
                got: seq,
            });
        }

        // payload
        let body = &input[8..len - 4];

        // crc32
        let crc = {
            let mut buf = [0; 4];
            buf.copy_from_slice(&input[len - 4..len]);
            u32::from_le_bytes(buf)
        };

        let valid_crc = {
            let mut digest = crc32::Digest::new(crc32::IEEE);
            digest.write(&len_bytes);
            digest.write(&seq_bytes);
            digest.write(body);
            digest.sum32()
        };
        if crc != valid_crc {
            return Err(TransportError::BadCrc {
                expected: valid_crc,
                got: crc,
            });
        }

        self.recv_seq += 1;
        output.extend_from_slice(body);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Returns a new full transport, `n` bytes of input data for it, and an empty output buffer.
    fn setup_pack(n: u32) -> (Full, Vec<u8>, Vec<u8>) {
        let input = (0..n).map(|x| (x & 0xff) as u8).collect();
        (Full::new(), input, Vec::new())
    }

    /// Returns the expected data after unpacking, a new full transport, input data and an empty output buffer.
    fn setup_unpack(n: u32) -> (Vec<u8>, Full, Vec<u8>, Vec<u8>) {
        let (mut transport, expected_output, mut input) = setup_pack(n);
        transport.pack(&expected_output, &mut input);

        (expected_output, Full::new(), input, Vec::new())
    }

    #[test]
    fn pack_empty() {
        let (mut transport, input, mut output) = setup_pack(0);
        transport.pack(&input, &mut output);

        assert_eq!(&output, &[12, 0, 0, 0, 0, 0, 0, 0, 38, 202, 141, 50]);
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

        assert_eq!(&output[..4], &[140, 0, 0, 0]);
        assert_eq!(&output[4..8], &[0, 0, 0, 0]);
        assert_eq!(&output[8..8 + input.len()], &input[..]);
        assert_eq!(&output[8 + input.len()..], &[134, 115, 149, 55]);
    }

    #[test]
    fn pack_twice() {
        let (mut transport, input, mut output) = setup_pack(128);
        transport.pack(&input, &mut output);
        output.clear();
        transport.pack(&input, &mut output);

        assert_eq!(&output[..4], &[140, 0, 0, 0]);
        assert_eq!(&output[4..8], &[1, 0, 0, 0]);
        assert_eq!(&output[8..8 + input.len()], &input[..]);
        assert_eq!(&output[8 + input.len()..], &[150, 9, 240, 74]);
    }

    #[test]
    fn unpack_small() {
        let mut transport = Full::new();
        let input = [0, 1, 2];
        let mut output = Vec::new();
        assert_eq!(
            transport.unpack(&input, &mut output),
            Err(TransportError::MissingBytes(4))
        );
    }

    #[test]
    fn unpack_normal() {
        let (expected_output, mut transport, input, mut output) = setup_unpack(128);
        transport.unpack(&input, &mut output).unwrap();
        assert_eq!(output, expected_output);
    }

    #[test]
    fn unpack_twice() {
        let (mut transport, input, mut packed) = setup_pack(128);
        let mut unpacked = Vec::new();
        transport.pack(&input, &mut packed);
        transport.unpack(&packed, &mut unpacked).unwrap();
        assert_eq!(input, unpacked);

        packed.clear();
        unpacked.clear();
        transport.pack(&input, &mut packed);
        transport.unpack(&packed, &mut unpacked).unwrap();
        assert_eq!(input, unpacked);
    }

    #[test]
    fn unpack_bad_crc() {
        let (_expected_output, mut transport, mut input, mut output) = setup_unpack(128);
        let last = input.len() - 1;
        input[last] ^= 0xff;
        assert_eq!(
            transport.unpack(&input, &mut output),
            Err(TransportError::BadCrc {
                expected: 932541318,
                got: 3365237638,
            })
        );
    }

    #[test]
    fn unpack_bad_seq() {
        let (mut transport, input, mut packed) = setup_pack(128);
        let mut unpacked = Vec::new();
        transport.pack(&input, &mut packed);
        packed.clear();
        transport.pack(&input, &mut packed);
        assert_eq!(
            transport.unpack(&packed, &mut unpacked),
            Err(TransportError::BadSeq {
                expected: 0,
                got: 1,
            })
        );
    }
}
