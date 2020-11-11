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
pub use encrypted::Encrypted;
use grammers_crypto as crypto;
use grammers_tl_types as tl;
pub use plain::Plain;
use std::fmt;

/// Results from the deserialization of a response.
pub struct Deserialization {
    /// Result bodies to Remote Procedure Calls.
    pub rpc_results: Vec<(MsgId, Result<Vec<u8>, RequestError>)>,
    /// Updates that came in the response.
    pub updates: Vec<Vec<u8>>,
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

    /// The error occured at the [transport level], making it impossible to
    /// deserialize any data. The absolute value indicates the HTTP error
    /// code. Some known, possible codes are:
    ///
    /// * 404, if the authorization key used was not found, meaning that the
    ///   server is not aware of the key used by the client, so it cannot be
    ///   used to securely communicate with it.
    ///
    /// * 429, if too many transport connections are established to the same
    ///   IP address in a too-short lapse of time.
    ///
    /// [transport level]: https://core.telegram.org/mtproto/mtproto-transports#transport-errors
    TransportError { code: i32 },

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
            Self::TransportError { code } => {
                write!(f, "transpot-level error, http status code: {}", code.abs())
            }
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

/// The error type reported by the server when a request is misused.
#[derive(Clone, Debug, PartialEq)]
pub struct RpcError {
    /// A numerical value similar to HTTP status codes.
    pub code: i32,

    /// The ASCII error name, normally in screaming snake case.
    pub name: String,

    /// If the error contained an additional value, it will be present here.
    pub value: Option<u32>,
}

impl std::error::Error for RpcError {}

impl fmt::Display for RpcError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "rpc error {}: {}", self.code, self.name)?;
        if let Some(value) = self.value {
            write!(f, " (value: {})", value)?;
        }
        Ok(())
    }
}

impl From<tl::types::RpcError> for RpcError {
    fn from(error: tl::types::RpcError) -> Self {
        // Extract the numeric value in the error, if any
        if let Some(value) = error
            .error_message
            .split(|c: char| !c.is_digit(10))
            .find(|s| !s.is_empty())
        {
            let mut to_remove = String::with_capacity(1 + value.len());
            to_remove.push('_');
            to_remove.push_str(value);
            Self {
                code: error.error_code,
                name: error.error_message.replace(&to_remove, ""),
                // Safe to unwrap, matched on digits
                value: Some(value.parse().unwrap()),
            }
        } else {
            Self {
                code: error.error_code,
                name: error.error_message.clone(),
                value: None,
            }
        }
    }
}

/// This error occurs when a Remote Procedure call was unsuccessful.
///
/// The request should be retransmited when this happens, unless the
/// variant is `InvalidParameters`.
#[derive(Clone, Debug, PartialEq)]
pub enum RequestError {
    /// The parameters used in the request were invalid and caused a
    /// Remote Procedure Call error.
    RpcError(RpcError),

    /// The call was dropped (cancelled), so the server will not process it.
    Dropped,

    /// The message sent to the server was invalid, and the request
    /// must be retransmitted.
    BadMessage {
        /// The code of the bad message error.
        code: i32,
    },

    /// The deserialization of the response that was meant to confirm this
    /// request failed, so while the server technically responded to the
    /// request its answer is useless as it could not be understood properly.
    Deserialize(DeserializeError),
}

impl std::error::Error for RequestError {}

impl fmt::Display for RequestError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::RpcError(error) => write!(f, "request error: {}", error),
            Self::Dropped => write!(f, "request error: request dropped"),
            Self::BadMessage { code } => write!(f, "request error: bad message (code {})", code),
            Self::Deserialize(error) => write!(f, "request error: {}", error),
        }
    }
}

impl From<DeserializeError> for RequestError {
    fn from(error: DeserializeError) -> Self {
        Self::Deserialize(error)
    }
}

impl From<tl::deserialize::Error> for RequestError {
    fn from(error: tl::deserialize::Error) -> Self {
        RequestError::from(DeserializeError::from(error))
    }
}

/// The trait used by the [Mobile Transport Protocol] to serialize outgoing
/// messages and deserialize incoming ones into proper responses.
///
/// [Mobile Transport Protocol]: https://core.telegram.org/mtproto/description
pub trait Mtp {
    /// Serializes one request to the internal buffer, which can be later retrieved by calling
    /// `finalize` after one or more `push` have been made.
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
    fn push(&mut self, request: &[u8]) -> Option<MsgId>;

    /// Finalizes the internal buffer of requests.
    ///
    /// Note that even if there are no requests to serialize, the protocol may
    /// produce data that has to be sent after deserializing incoming messages.
    fn finalize(&mut self) -> Vec<u8>;

    /// Deserializes a single incoming message payload into zero or more responses.
    fn deserialize(&mut self, payload: &[u8]) -> Result<Deserialization, DeserializeError>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn check_rpc_error_parsing() {
        assert_eq!(
            RpcError::from(tl::types::RpcError {
                error_code: 400,
                error_message: "CHAT_INVALID".into(),
            }),
            RpcError {
                code: 400,
                name: "CHAT_INVALID".into(),
                value: None
            }
        );

        assert_eq!(
            RpcError::from(tl::types::RpcError {
                error_code: 420,
                error_message: "FLOOD_WAIT_31".into(),
            }),
            RpcError {
                code: 420,
                name: "FLOOD_WAIT".into(),
                value: Some(31)
            }
        );

        assert_eq!(
            RpcError::from(tl::types::RpcError {
                error_code: 500,
                error_message: "INTERDC_2_CALL_ERROR".into(),
            }),
            RpcError {
                code: 500,
                name: "INTERDC_CALL_ERROR".into(),
                value: Some(2)
            }
        );
    }
}
