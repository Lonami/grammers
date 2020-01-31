// Copyright 2020 - developers of the `grammers` project.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// https://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or https://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.
use crate::transports::{LengthTooLong, Transport};
use std::io::{Error, ErrorKind, Read, Result, Write};

/// The lightest MTProto transport protocol available. This is an
/// implementation of the [abridged transport].
///
/// * Overhead: very small.
/// * Minimum envelope length: 1 byte.
/// * Maximum envelope length: 4 bytes.
///
/// [abridged transport]: https://core.telegram.org/mtproto/mtproto-transports#abridged
pub struct TransportAbridged;

impl TransportAbridged {
    /// Creates a new instance of a `TransportAbridged`.
    pub fn new() -> Self {
        Self
    }
}

impl Transport for TransportAbridged {
    fn send<W: Write>(&mut self, channel: &mut W, payload: &[u8]) -> Result<()> {
        let len = payload.len() / 4;
        if len < 127 {
            channel.write_all(&[len as u8])?;
        } else {
            let mut len = len.to_le_bytes();
            // shift to the right to make room in the first byte
            for i in (1..len.len()).rev() {
                len[i] = len[i - 1];
            }
            len[0] = 0x7f;
            channel.write_all(&len)?;
        }

        channel.write_all(payload)?;
        Ok(())
    }

    fn receive_into<R: Read>(&mut self, channel: &mut R, buffer: &mut Vec<u8>) -> Result<()> {
        let len = {
            let mut buf = [0; 1];
            channel.read_exact(&mut buf)?;
            buf[0]
        };

        let len = if len < 127 {
            len as u32
        } else {
            let mut buf = [0; 3];
            channel.read_exact(&mut buf)?;

            let mut len = [0; 4];
            len[..buf.len()].copy_from_slice(&buf);
            u32::from_le_bytes(len)
        };

        let len = len * 4;

        if len > Self::MAXIMUM_DATA {
            return Err(Error::new(ErrorKind::InvalidInput, LengthTooLong { len }));
        }

        buffer.resize(len as usize, 0);
        channel.read_exact(buffer)?;
        Ok(())
    }
}
