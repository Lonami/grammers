// Copyright 2020 - developers of the `grammers` project.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// https://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or https://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.
use grammers_mtproto::{authentication, mtp, transport};
use grammers_tl_types as tl;
use std::fmt;
use std::io;

#[derive(Debug)]
pub enum ReadError {
    Io(io::Error),
    Transport(transport::Error),
    Deserialize(mtp::DeserializeError),
}

impl std::error::Error for ReadError {}

impl fmt::Display for ReadError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(err) => write!(f, "read error, IO failed: {}", err),
            Self::Transport(err) => write!(f, "read error, transport-level: {}", err),
            Self::Deserialize(err) => write!(f, "read error, bad response: {}", err),
        }
    }
}

impl From<io::Error> for ReadError {
    fn from(error: io::Error) -> Self {
        Self::Io(error)
    }
}

impl From<transport::Error> for ReadError {
    fn from(error: transport::Error) -> Self {
        Self::Transport(error)
    }
}

impl From<mtp::DeserializeError> for ReadError {
    fn from(error: mtp::DeserializeError) -> Self {
        Self::Deserialize(error)
    }
}

impl From<tl::deserialize::Error> for ReadError {
    fn from(error: tl::deserialize::Error) -> Self {
        Self::Deserialize(error.into())
    }
}

/// This error occurs when a Remote Procedure call was unsuccessful.
///
/// The request should be retransmited when this happens, unless the
/// variant is `InvalidParameters`.
#[derive(Debug)]
pub enum InvocationError {
    /// The request invocation failed because it was invalid or the server
    /// could not process it successfully.
    Rpc(mtp::RpcError),

    /// The request was cancelled or dropped, and the results won't arrive.
    Dropped,

    /// The error occured while reading the response.
    Read(ReadError),
}

impl std::error::Error for InvocationError {}

impl fmt::Display for InvocationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Rpc(err) => write!(f, "request error: {}", err),
            Self::Dropped => write!(f, "request error: dropped (cancelled)"),
            Self::Read(err) => write!(f, "request error: {}", err),
        }
    }
}

impl From<ReadError> for InvocationError {
    fn from(error: ReadError) -> Self {
        Self::Read(error)
    }
}

impl From<mtp::DeserializeError> for InvocationError {
    fn from(error: mtp::DeserializeError) -> Self {
        Self::from(ReadError::from(error))
    }
}

impl From<tl::deserialize::Error> for InvocationError {
    fn from(error: tl::deserialize::Error) -> Self {
        Self::from(ReadError::from(error))
    }
}

impl InvocationError {
    /// Matches on the name of the RPC error (case-sensitive).
    ///
    /// Useful in `match` arm guards. A single trailing or leading asterisk (`'*'`) is allowed,
    /// and will instead check if the error name starts (or ends with) the input parameter.
    ///
    /// If the error is not a RPC error, returns `false`.
    ///
    /// # Examples
    ///
    /// ```
    /// # let request_result = Result::<(), _>::Err(grammers_mtsender::InvocationError::Rpc(
    /// #     grammers_mtproto::mtp::RpcError { code: 400, name: "PHONE_CODE_INVALID".to_string(), value: None, caused_by: None }));
    /// #
    /// match request_result {
    ///     Err(err) if err.is("SESSION_PASSWORD_NEEDED") => panic!(),
    ///     Err(err) if err.is("PHONE_CODE_*") => {},
    ///     _ => panic!()
    /// }
    /// ```
    #[inline]
    pub fn is(&self, rpc_error: &str) -> bool {
        match self {
            Self::Rpc(rpc) => rpc.is(rpc_error),
            _ => false,
        }
    }
}

/// This error occurs when the process to generate an authorization key fails.
#[derive(Debug)]
pub enum AuthorizationError {
    /// The generation failed because the generation process went wrong.
    Gen(authentication::Error),

    /// The generation failed because invoking a request failed.
    Invoke(InvocationError),
}

impl std::error::Error for AuthorizationError {}

impl fmt::Display for AuthorizationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Gen(err) => write!(f, "authorization error: {}", err),
            Self::Invoke(err) => write!(f, "authorization error: {}", err),
        }
    }
}

impl From<authentication::Error> for AuthorizationError {
    fn from(error: authentication::Error) -> Self {
        Self::Gen(error)
    }
}

impl From<InvocationError> for AuthorizationError {
    fn from(error: InvocationError) -> Self {
        Self::Invoke(error)
    }
}

impl From<io::Error> for AuthorizationError {
    fn from(error: io::Error) -> Self {
        // TODO not entirely happy with some of these error chains
        // might need to "flatten" them to not depend on layers so deep
        Self::from(InvocationError::from(ReadError::from(error)))
    }
}
