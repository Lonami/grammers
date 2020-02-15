// Copyright 2020 - developers of the `grammers` project.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// https://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or https://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

//! Errors that can occur when using the library's functions.

use std::error::Error;
use std::fmt;
use std::io;

use grammers_crypto::DecryptionError;
use grammers_tl_types as tl;

/// The error type for the deserialization of server messages.
#[derive(Debug)]
pub enum DeserializeError {
    /// The authorization key is missing, so data cannot be decrypted.
    MissingAuthKey,

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

    /// The received buffer is too small to contain a valid response message.
    MessageBufferTooSmall,

    /// The server responded with compressed data which we failed to decompress.
    DecompressionFailed,

    /// Reading from the buffer failed in some way.
    BufferError(io::Error),

    /// Attempting to decrypt the message failed in some way.
    DecryptionError(DecryptionError),
}

impl Error for DeserializeError {}

impl fmt::Display for DeserializeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match *self {
            Self::MissingAuthKey => write!(f, "missing auth key"),
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
            Self::BufferError(ref error) => write!(f, "failed to deserialize message: {}", error),
            Self::DecryptionError(ref error) => write!(f, "failed to decrypt message: {}", error),
        }
    }
}

impl From<io::Error> for DeserializeError {
    fn from(error: io::Error) -> Self {
        Self::BufferError(error)
    }
}

impl From<DecryptionError> for DeserializeError {
    fn from(error: DecryptionError) -> Self {
        Self::DecryptionError(error)
    }
}

/// The error type for the serialization of outgoing messages.
#[derive(Debug)]
pub enum SerializeError {
    /// The authorization key is missing, so data cannot be encrypted.
    MissingAuthKey,

    /// The serialization would be considered invalid because the request
    /// payload is too large and cannot possibly be sent. Telegram would
    /// forcibly close the connection if it was ever sent.
    PayloadTooLarge,
}

impl Error for SerializeError {}

impl fmt::Display for SerializeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match *self {
            Self::MissingAuthKey => write!(f, "missing auth key"),
            Self::PayloadTooLarge => write!(f, "request payload too large"),
        }
    }
}

/// This error occurs when a Remote Procedure call was unsuccessful.
///
/// The request should be retransmited when this happens, unless the
/// variant is `InvalidParameters`.
#[derive(Debug)]
pub enum RequestError {
    /// The parameters used in the request were invalid and caused a
    /// Remote Procedure Call error.
    RPCError(RPCError),

    /// The call was dropped (cancelled), so the server will not process it.
    Dropped,

    /// The message sent to the server was invalid, and the request
    /// must be retransmitted.
    BadMessage {
        /// The code of the bad message error.
        code: i32,
    },
}

impl RequestError {
    pub fn should_retransmit(&self) -> bool {
        match self {
            Self::RPCError(_) => false,
            _ => true,
        }
    }
}

/// The error type reported by the server when a request is misused.
#[derive(Debug, PartialEq)]
pub struct RPCError {
    /// A numerical value similar to HTTP status codes.
    pub code: i32,

    /// The ASCII error name, normally in screaming snake case.
    pub name: String,

    /// If the error contained an additional value, it will be present here.
    pub value: Option<u32>,
}

impl Error for RPCError {}

impl fmt::Display for RPCError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "rpc error {}: {}", self.code, self.name)?;
        if let Some(value) = self.value {
            write!(f, " (value: {})", value)?;
        }
        Ok(())
    }
}

impl From<tl::types::RpcError> for RPCError {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn check_rpc_error_parsing() {
        assert_eq!(
            RPCError::from(tl::types::RpcError {
                error_code: 400,
                error_message: "CHAT_INVALID".into(),
            }),
            RPCError {
                code: 400,
                name: "CHAT_INVALID".into(),
                value: None
            }
        );

        assert_eq!(
            RPCError::from(tl::types::RpcError {
                error_code: 420,
                error_message: "FLOOD_WAIT_31".into(),
            }),
            RPCError {
                code: 420,
                name: "FLOOD_WAIT".into(),
                value: Some(31)
            }
        );

        assert_eq!(
            RPCError::from(tl::types::RpcError {
                error_code: 500,
                error_message: "INTERDC_2_CALL_ERROR".into(),
            }),
            RPCError {
                code: 500,
                name: "INTERDC_CALL_ERROR".into(),
                value: Some(2)
            }
        );
    }
}
