// Copyright 2020 - developers of the `grammers` project.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// https://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or https://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.
use super::{Deserialization, DeserializeError, Mtp};
use crate::MsgId;
use grammers_tl_types::{Cursor, Deserializable, Serializable};
use std::mem;

/// An implementation of the [Mobile Transport Protocol] for plaintext
/// (unencrypted) messages.
///
/// The reason to separate the plaintext and encrypted implementations
/// for serializing messages is that, even though they are similar, the
/// benefits outweight some minor code reuse.
///
/// This way, the encryption key for [`Mtp`] is mandatory so errors
/// for trying to encrypt data without a key are completely eliminated.
///
/// Also, the plaintext part of the protocol does not need to deal with
/// the complexity of the full protocol once encrypted messages are used,
/// so being able to keep a simpler implementation separate is a bonus.
///
/// [Mobile Transport Protocol]: https://core.telegram.org/mtproto
/// [`Mtp`]: struct.Mtp.html
#[non_exhaustive]
pub struct Plain {
    buffer: Vec<u8>,
}

impl Plain {
    pub fn new() -> Self {
        Self { buffer: Vec::new() }
    }
}

impl Mtp for Plain {
    /// Wraps a request's data into a plain message (also known as
    /// [unencrypted messages]), and returns its serialized contents.
    ///
    /// Plain messages may be used for requests that don't require an
    /// authorization key to be present, such as those needed to generate
    /// the authorization key itself.
    ///
    /// [unencrypted messages]: https://core.telegram.org/mtproto/description#unencrypted-message
    fn push(&mut self, request: &[u8]) -> Option<MsgId> {
        if !self.buffer.is_empty() {
            return None;
        }

        0i64.serialize(&mut self.buffer); // auth_key_id = 0

        // Even though https://core.telegram.org/mtproto/samples-auth_key
        // seems to imply the `msg_id` has to follow some rules, there is
        // no need to generate a valid `msg_id`, it seems. Just use `0`.
        0i64.serialize(&mut self.buffer); // message_id

        (request.len() as i32).serialize(&mut self.buffer); // message_data_length
        self.buffer.extend_from_slice(request); // message_data

        Some(MsgId(0))
    }

    fn finalize(&mut self) -> Vec<u8> {
        mem::take(&mut self.buffer)
    }

    /// Validates that the returned data is a correct plain message, and
    /// if it is, the method returns the inner contents of the message.
    ///
    /// [`serialize_plain_message`]: #method.serialize_plain_message
    fn deserialize(&mut self, payload: &[u8]) -> Result<Deserialization, DeserializeError> {
        crate::utils::check_message_buffer(payload)?;

        let mut buf = Cursor::from_slice(payload);
        let auth_key_id = i64::deserialize(&mut buf)?;
        if auth_key_id != 0 {
            return Err(DeserializeError::BadAuthKey {
                got: auth_key_id,
                expected: 0,
            });
        }

        let msg_id = i64::deserialize(&mut buf)?;
        // We can't validate it's close to our system time because our sytem
        // time may be wrong at this point (it only matters once encrypted
        // communication begins). However, we can validate the following:
        //
        // > server message identifiers modulo 4 yield 1 if
        // > the message is a response to a client message
        // https://core.telegram.org/mtproto/description#message-identifier-msg-id
        if msg_id <= 0 || (msg_id % 4) != 1 {
            return Err(DeserializeError::BadMessageId { got: msg_id });
        }

        let len = i32::deserialize(&mut buf)?;
        if len <= 0 {
            return Err(DeserializeError::NegativeMessageLength { got: len });
        }
        if (20 + len) as usize > payload.len() {
            return Err(DeserializeError::TooLongMessageLength {
                got: len as usize,
                max_length: payload.len() - 20,
            });
        }

        Ok(Deserialization {
            rpc_results: vec![(MsgId(0), Ok(payload[20..20 + len as usize].into()))],
            updates: Vec::new(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const REQUEST: &[u8] = b"Hey!";

    #[test]
    fn ensure_finalize_clears_buffer() {
        let mut mtp = Plain::new();

        mtp.push(REQUEST);
        assert_eq!(mtp.finalize().len(), 24);

        mtp.push(REQUEST);
        assert_eq!(mtp.finalize().len(), 24);
    }

    #[test]
    fn ensure_only_one_push_allowed() {
        let mut mtp = Plain::new();

        assert!(mtp.push(REQUEST).is_some());
        assert!(mtp.push(REQUEST).is_none());
    }
}
