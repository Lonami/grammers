// Copyright 2020 - developers of the `grammers` project.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// https://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or https://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.
use crate::transports::Transport;

/// A light MTProto transport protocol available that guarantees data padded
/// to 4 bytes. This is an implementation of the [intermediate transport].
///
/// * Overhead: small.
/// * Minimum envelope length: 4 bytes.
/// * Maximum envelope length: 4 bytes.
///
/// [intermediate transport]: https://core.telegram.org/mtproto/mtproto-transports#intermediate
#[derive(Default)]
pub struct TransportIntermediate;

impl TransportIntermediate {
    /// Creates a new instance of a `TransportIntermediate`.
    pub fn new() -> Self {
        Self
    }
}

/// Serializes the input payload as follows:
///
/// ```text
/// +----+----...----+
/// | len|  payload  |
/// +----+----...----+
///  ^^^^ 4 bytes
/// ```
impl Transport for TransportIntermediate {
    const MAX_OVERHEAD: usize = 4;

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

    fn read<'a>(&mut self, input: &'a [u8]) -> Result<&'a [u8], usize> {
        if input.len() < 4 {
            return Err(4);
        }

        let len = {
            let mut buf = [0; 4];
            buf.copy_from_slice(&input[0..4]);
            u32::from_le_bytes(buf)
        } as usize
            + 4;

        if input.len() < len {
            return Err(len);
        }

        let output = &input[4..len - 4];
        Ok(output)
    }
}
