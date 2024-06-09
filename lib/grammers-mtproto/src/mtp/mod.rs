// Copyright 2020 - developers of the `grammers` project.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// https://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or https://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

//! Implementation of the [Mobile Transport Protocol]. This layer is
//! responsible for converting zero or more input requests into outgoing
//! messages, and to process the response.
//!
//! A distinction between plain and encrypted is made for simplicity (the
//! plain hardly requires to process any state) and to help prevent invalid
//! states (encrypted communication cannot be made without an authorization
//! key).
//!
//! [Mobile Transport Protocol]: https://core.telegram.org/mtproto/description
mod encrypted;
mod plain;

use crate::MsgId;
use crypto::RingBuffer;
pub use encrypted::{
    Encrypted, ENCRYPTED_PACKET_HEADER_LEN, MAX_TRANSPORT_HEADER_LEN, MESSAGE_CONTAINER_HEADER_LEN,
    PLAIN_PACKET_HEADER_LEN,
};
use grammers_crypto as crypto;
use grammers_tl_types as tl;
pub use plain::Plain;
use std::fmt;

pub struct RpcResult {
    pub msg_id: MsgId,
    pub body: Vec<u8>,
}

pub struct RpcResultError {
    pub msg_id: MsgId,
    pub error: tl::types::RpcError,
}

pub struct BadMessage {
    pub msg_id: MsgId,
    pub code: i32,
}

pub struct DeserializationFailure {
    pub msg_id: MsgId,
    pub error: DeserializeError,
}

/// Results from the deserialization of a response.
pub enum Deserialization {
    Update(Vec<u8>),
    RpcResult(RpcResult),
    RpcError(RpcResultError),
    BadMessage(BadMessage),
    Failure(DeserializationFailure),
}

impl BadMessage {
    pub fn description(&self) -> &'static str {
        // https://core.telegram.org/mtproto/service_messages_about_messages
        match self.code {
            16 => "msg_id too low",
            17 => "msg_id too high",
            18 => "incorrect two lower order msg_id bits; this is a bug",
            19 => "container msg_id is the same as msg_id of a previously received message; this is a bug",
            20 => "message too old",
            32 => "msg_seqno too low",
            33 => "msg_seqno too high",
            34 => "an even msg_seqno expected; this may be a bug",
            35 => "odd msg_seqno expected; this may be a bug",
            48 => "incorrect server salt",
            64 => "invalid container; this is likely a bug",
            _ => "unknown explanation; please report this issue",
        }
    }

    pub fn retryable(&self) -> bool {
        [16, 17, 48].contains(&self.code)
    }

    pub fn fatal(&self) -> bool {
        !self.retryable() && ![32, 33].contains(&self.code)
    }
}

/// The error type for the deserialization of server messages.
#[derive(Clone, Debug, PartialEq)]
pub enum DeserializeError {
    /// The server's authorization key did not match our expectations.
    BadAuthKey { got: i64, expected: i64 },

    /// The server's message ID did not match our expectations.
    BadMessageId { got: i64 },

    /// The server's message length was not strictly positive.
    NegativeMessageLength { got: i32 },

    /// The server's message length was past the buffer.
    TooLongMessageLength { got: usize, max_length: usize },

    /// The received buffer is too small to contain a valid response message,
    /// or the response seemed valid at first but trying to deserialize it
    /// proved the buffer to be too small.
    MessageBufferTooSmall,

    /// The server responded with compressed data which we failed to decompress.
    DecompressionFailed,

    /// While deserializing the response types one of them had a constructor
    /// that did not match our expectations. The invalid ID is contained
    /// within this variant.
    UnexpectedConstructor { id: u32 },

    /// Attempting to decrypt the message failed in some way.
    DecryptionError(crypto::Error),
}

impl std::error::Error for DeserializeError {}

impl fmt::Display for DeserializeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match *self {
            Self::BadAuthKey { got, expected } => write!(
                f,
                "bad server auth key (got {}, expected {})",
                got, expected
            ),
            Self::BadMessageId { got } => write!(f, "bad server message id (got {})", got),
            Self::NegativeMessageLength { got } => {
                write!(f, "bad server message length (got {})", got)
            }
            Self::TooLongMessageLength { got, max_length } => write!(
                f,
                "bad server message length (got {}, when at most it should be {})",
                got, max_length
            ),
            Self::MessageBufferTooSmall => write!(
                f,
                "server responded with a payload that's too small to fit a valid message"
            ),
            Self::DecompressionFailed => write!(f, "failed to decompress server's data"),
            Self::UnexpectedConstructor { id } => write!(f, "unexpected constructor: {:08x}", id),
            Self::DecryptionError(ref error) => write!(f, "failed to decrypt message: {}", error),
        }
    }
}

impl From<tl::deserialize::Error> for DeserializeError {
    fn from(error: tl::deserialize::Error) -> Self {
        use tl::deserialize::Error as Err;

        match error {
            Err::UnexpectedEof => DeserializeError::MessageBufferTooSmall,
            Err::UnexpectedConstructor { id } => DeserializeError::UnexpectedConstructor { id },
        }
    }
}

impl From<crypto::Error> for DeserializeError {
    fn from(error: crypto::Error) -> Self {
        Self::DecryptionError(error)
    }
}

/// The trait used by the [Mobile Transport Protocol] to serialize outgoing
/// messages and deserialize incoming ones into proper responses.
///
/// [Mobile Transport Protocol]: https://core.telegram.org/mtproto/description
pub trait Mtp {
    /// Serializes one request to the input buffer.
    /// The same buffer should be used until `finalize` is called.
    ///
    /// Returns the message ID assigned the request if it was serialized, or `None` if the buffer
    /// is full and cannot hold more requests.
    ///
    /// # Panics
    ///
    /// The method panics if the body length is not padded to 4 bytes. The
    /// serialization of requests will always be correctly padded, so adding
    /// an error case for this rare case (impossible with the expected inputs)
    /// would simply be unnecessary.
    ///
    /// The method also panics if the body length is too large for similar
    /// reasons. It is not reasonable to construct huge requests (although
    /// possible) because they would likely fail with a RPC error anyway,
    /// so we avoid another error case by simply panicking.
    ///
    /// The definition of "too large" is roughly 1MB, so as long as the
    /// payload is below that mark, it's safe to call.
    fn push(&mut self, buffer: &mut RingBuffer<u8>, request: &[u8]) -> Option<MsgId>;

    /// Finalizes the buffer of requests.
    ///
    /// Note that even if there are no requests to serialize, the protocol may
    /// produce data that has to be sent after deserializing incoming messages.
    ///
    /// The buffer may remain empty if there are no actions to take.
    ///
    /// When at least one message is serialized, the last generated `MsgId` is returned.
    /// This will either belong to the container (if used) or the last serialized message.
    fn finalize(&mut self, buffer: &mut RingBuffer<u8>) -> Option<MsgId>;

    /// Deserializes a single incoming message payload into zero or more responses.
    fn deserialize(&mut self, payload: &[u8]) -> Result<Vec<Deserialization>, DeserializeError>;

    /// Reset the state, as if a new instance was just created.
    fn reset(&mut self);
}
