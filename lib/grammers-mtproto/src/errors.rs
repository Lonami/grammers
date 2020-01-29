//! This module contains all the custom errors used by the library.

use std::error::Error;
use std::fmt;
use std::io;

/// This error that occurs while enqueueing requests.
#[derive(Debug)]
pub enum EnqueueError {
    /// The request payload is too large and cannot possibly be sent.
    /// Telegram would forcibly close the connection if it was ever sent.
    PayloadTooLarge,

    /// Well-formed data must be padded to 4 bytes.
    IncorrectPadding,
}

impl Error for EnqueueError {}

impl fmt::Display for EnqueueError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match *self {
            Self::PayloadTooLarge => write!(f, "the payload is too large and cannot be sent"),
            Self::IncorrectPadding => write!(f, "the data is not padded correctly"),
        }
    }
}

impl From<EnqueueError> for io::Error {
    fn from(error: EnqueueError) -> Self {
        io::Error::new(io::ErrorKind::InvalidData, error)
    }
}

/// This error occurs while deserializing server messages.
#[derive(Debug)]
pub enum DeserializeError {
    /// The server's authorization key did not match our expectations.
    BadAuthKey { got: i64, expected: i64 },

    /// The server's message ID did not match our expectations.
    BadMessageId { got: i64 },

    /// The server's message length was not strictly positive.
    NegativeMessageLength { got: i32 },

    /// The server's message length was past the buffer.
    TooLongMessageLength { got: usize, max_length: usize },

    /// The server returned a negative HTTP error code and not a message.
    HTTPErrorCode { code: i32 },

    /// The received buffer is too small to contain a valid response message.
    MessageBufferTooSmall,

    /// The server responded with compressed data which we failed to decompress.
    DecompressionFailed,
}

impl Error for DeserializeError {}

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
            Self::HTTPErrorCode { code } => {
                write!(f, "server responded with negative http status: {}", code)
            }
            Self::MessageBufferTooSmall => write!(
                f,
                "server responded with a payload that's too small to fit a valid message"
            ),
            Self::DecompressionFailed => write!(f, "failed to decompress server's data"),
        }
    }
}

impl From<DeserializeError> for io::Error {
    fn from(error: DeserializeError) -> Self {
        io::Error::new(io::ErrorKind::InvalidData, error)
    }
}
