// Copyright 2020 - developers of the `grammers` project.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// https://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or https://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.
use std::error::Error;
use std::fmt;
use std::io;

use grammers_crypto::auth_key::generation::AuthKeyGenError;
use grammers_mtproto::errors::{DeserializeError, RPCError, SerializeError};

/// This error occurs when the process to generate an authorization key fails.
#[derive(Debug)]
pub enum AuthorizationError {
    /// The generation failed due to network problems.
    IO(io::Error),

    /// The generation failed because the generation process went wrong.
    Gen(AuthKeyGenError),

    /// The generation failed because invoking a request failed.
    Invocation(InvocationError),
}

impl Error for AuthorizationError {}

impl fmt::Display for AuthorizationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::IO(err) => write!(f, "auth key gen error, IO failed: {}", err),
            Self::Gen(err) => write!(f, "auth key gen error, process failed: {}", err),
            Self::Invocation(err) => write!(f, "auth key gen error, bad invoke: {}", err),
        }
    }
}

impl From<io::Error> for AuthorizationError {
    fn from(error: io::Error) -> Self {
        Self::IO(error)
    }
}

impl From<AuthKeyGenError> for AuthorizationError {
    fn from(error: AuthKeyGenError) -> Self {
        Self::Gen(error)
    }
}

impl From<InvocationError> for AuthorizationError {
    fn from(error: InvocationError) -> Self {
        Self::Invocation(error)
    }
}

/// This error occurs when a Remote Procedure call was unsuccessful.
///
/// The request should be retransmited when this happens, unless the
/// variant is `InvalidParameters`.
#[derive(Debug)]
pub enum InvocationError {
    /// The request invocation failed due to network problems.
    ///
    /// This includes being unable to send malformed packets to the server
    /// (such as a packet being large) because attempting to send those would
    /// cause the server to disconnect.
    ///
    /// This also includes being unable to deserialize incoming messages,
    /// simply because it's more convenient to have those errors here.
    IO(io::Error),

    /// The request invocation failed because it was invalid or the server
    /// could not process it successfully.
    RPC(RPCError),

    /// The request was cancelled or dropped, and the results won't arrive.
    Dropped,

    /// The error occured during the deserialization of the response.
    Deserialize(DeserializeError),

    /// The error occured during the serialization of the request.
    Serialize(SerializeError),
}

impl Error for InvocationError {}

impl fmt::Display for InvocationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::IO(err) => write!(f, "request error, IO failed: {}", err),
            Self::RPC(err) => write!(f, "request error, invoking failed: {}", err),
            Self::Dropped => write!(f, "request was dropped (cancelled)"),
            Self::Deserialize(err) => write!(f, "request error, bad response: {}", err),
            Self::Serialize(err) => write!(f, "request error, bad request: {}", err),
        }
    }
}

impl From<io::Error> for InvocationError {
    fn from(error: io::Error) -> Self {
        Self::IO(error)
    }
}

impl From<DeserializeError> for InvocationError {
    fn from(error: DeserializeError) -> Self {
        Self::Deserialize(error)
    }
}

impl From<SerializeError> for InvocationError {
    fn from(error: SerializeError) -> Self {
        Self::Serialize(error)
    }
}

impl From<RPCError> for InvocationError {
    fn from(error: RPCError) -> Self {
        Self::RPC(error)
    }
}
