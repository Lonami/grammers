// Copyright 2020 - developers of the `grammers` project.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// https://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or https://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.
use crate::transports::Transport;

/// The lightest MTProto transport protocol available. This is an
/// implementation of the [abridged transport].
///
/// * Overhead: very small.
/// * Minimum envelope length: 1 byte.
/// * Maximum envelope length: 4 bytes.
///
/// [abridged transport]: https://core.telegram.org/mtproto/mtproto-transports#abridged
#[derive(Default)]
pub struct TransportAbridged;

impl TransportAbridged {
    /// Creates a new instance of a `TransportAbridged`.
    pub fn new() -> Self {
        Self
    }
}

/// Serializes the input payload as follows.
///
/// When the length is small enough:
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
impl Transport for TransportAbridged {
    const MAX_OVERHEAD: usize = 4;

    fn write_into<'a>(&mut self, input: &[u8], output: &mut [u8]) -> Result<usize, usize> {
        let output_len;
        let len = input.len() / 4;
        if len < 127 {
            output_len = input.len() + 1;
            if output.len() < output_len {
                return Err(len);
            }

            output[0] = len as u8;
        } else {
            output_len = input.len() + 4;
            if output.len() < output_len {
                return Err(len);
            }

            output[0] = 0x7f;
            output[1..4].copy_from_slice(&len.to_le_bytes()[..3]);
        }

        output[output_len - input.len()..output_len].copy_from_slice(input);
        Ok(output_len)
    }

    fn read<'a>(&mut self, input: &'a [u8]) -> Result<&'a [u8], usize> {
        if input.len() < 1 {
            return Err(1);
        }

        let header_len;
        let len = input[0];
        let len = if len < 127 {
            header_len = 1;
            len as u32
        } else {
            if input.len() < 4 {
                return Err(4);
            }

            header_len = 4;
            let mut len = [0; 4];
            len[..3].copy_from_slice(&input[1..4]);
            u32::from_le_bytes(len)
        };

        let len = len as usize * 4;
        if input.len() < header_len + len {
            return Err(header_len + len);
        }

        Ok(&input[header_len..header_len + len])
    }
}
