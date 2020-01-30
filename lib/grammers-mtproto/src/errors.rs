//! Errors that can occur when using the library's functions.

use std::error::Error;
use std::fmt;
use std::io;

/// The error type for enqueueing requests.
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

/// The error type for the deserialization of server messages.
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
            Self::TransportError { code } => {
                write!(f, "transpot-level error, http status code: {}", code.abs())
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
