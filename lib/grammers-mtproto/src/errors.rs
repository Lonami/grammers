//! Errors that can occur when using the library's functions.

use std::error::Error;
use std::fmt;
use std::io;

use grammers_tl_types as tl;

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

/// This error occurs when a Remote Procedure call was unsuccessful.
///
/// The request should be retransmited when this happens, unless the
/// variant is `InvalidParameters`.
pub enum RequestError {
    /// The parameters used in the request were invalid and caused a
    /// Remote Procedure Call error.
    RPCError(RPCError),

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
    code: i32,

    /// The ASCII error name, normally in screaming snake case.
    name: String,

    /// If the error contained an additional value, it will be present here.
    value: Option<u32>,
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

impl From<RPCError> for io::Error {
    fn from(error: RPCError) -> Self {
        Self::new(io::ErrorKind::InvalidData, error)
    }
}

impl From<tl::types::RpcError> for RPCError {
    fn from(error: tl::types::RpcError) -> Self {
        // Extract the numeric value in the error, if any
        if let Some(value) = error
            .error_message
            .split(|c: char| !c.is_digit(10))
            .filter(|s| !s.is_empty())
            .next()
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
