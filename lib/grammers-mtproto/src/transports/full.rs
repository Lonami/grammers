// Copyright 2020 - developers of the `grammers` project.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// https://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or https://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.
use crate::transports::{Decoder, Encoder};
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
///
/// [full transport]: https://core.telegram.org/mtproto/mtproto-transports#full
pub fn full_transport() -> (FullEncoder, FullDecoder) {
    (FullEncoder { send_counter: 0 }, FullDecoder {})
}

pub struct FullEncoder {
    send_counter: u32,
}

#[non_exhaustive]
pub struct FullDecoder {}

impl Encoder for FullEncoder {
    const MAX_OVERHEAD: usize = 12;

    fn write_into<'a>(&mut self, input: &[u8], output: &mut [u8]) -> Result<usize, usize> {
        // payload len + length itself (4 bytes) + send counter (4 bytes) + crc32 (4 bytes)
        let len = input.len() + 4 + 4 + 4;
        if output.len() < len {
            return Err(len);
        }

        let len_bytes = (len as u32).to_le_bytes();
        let counter = self.send_counter.to_le_bytes();

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

        self.send_counter += 1;
        Ok(len)
    }
}

impl Decoder for FullDecoder {
    fn read<'a>(&mut self, input: &'a [u8]) -> Result<&'a [u8], usize> {
        // TODO the input and output len can probably be abstracted away
        //      ("minimal input" and "calculate output len")
        // Need 4 bytes for the initial length
        if input.len() < 4 {
            return Err(4);
        }

        // payload len
        let mut len_data = [0; 4];
        len_data.copy_from_slice(&input[0..4]);
        let len = u32::from_le_bytes(len_data) as usize;
        if input.len() < len {
            return Err(len);
        }

        // receive counter
        // TODO probably validate counter
        let mut counter_data = [0; 4];
        counter_data.copy_from_slice(&input[4..8]);
        let _counter = u32::from_le_bytes(counter_data);

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
            // TODO maybe with a `type TransportError` and return
            //      `Err(MissingBytes | TransportError)`
            unimplemented!("return InvalidCrc32 error")
        }

        Ok(output)
    }
}
