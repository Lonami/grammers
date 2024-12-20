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

impl Clone for ReadError {
    fn clone(&self) -> Self {
        match self {
            Self::Io(e) => Self::Io(
                e.raw_os_error()
                    .map(io::Error::from_raw_os_error)
                    .unwrap_or_else(|| io::Error::new(e.kind(), e.to_string())),
            ),
            Self::Transport(e) => Self::Transport(e.clone()),
            Self::Deserialize(e) => Self::Deserialize(e.clone()),
        }
    }
}

impl fmt::Display for ReadError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(err) => write!(f, "read error, IO failed: {err}"),
            Self::Transport(err) => write!(f, "read error, transport-level: {err}"),
            Self::Deserialize(err) => write!(f, "read error, bad response: {err}"),
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

/// The error type reported by the server when a request is misused.
#[derive(Clone, Debug, PartialEq)]
pub struct RpcError {
    /// A numerical value similar to HTTP status codes.
    pub code: i32,

    /// The ASCII error name, normally in screaming snake case.
    pub name: String,

    /// If the error contained an additional value, it will be present here.
    pub value: Option<u32>,

    /// The constructor identifier of the request that triggered this error.
    /// Won't be present if the error was artificially constructed.
    pub caused_by: Option<u32>,
}

impl std::error::Error for RpcError {}

impl fmt::Display for RpcError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "rpc error {}: {}", self.code, self.name)?;
        if let Some(caused_by) = self.caused_by {
            write!(f, " caused by {}", tl::name_for_id(caused_by))?;
        }
        if let Some(value) = self.value {
            write!(f, " (value: {value})")?;
        }
        Ok(())
    }
}

impl From<tl::types::RpcError> for RpcError {
    fn from(error: tl::types::RpcError) -> Self {
        // Extract the numeric value in the error, if any
        if let Some(value) = error
            .error_message
            .split(|c: char| !c.is_ascii_digit())
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
                caused_by: None,
            }
        } else {
            Self {
                code: error.error_code,
                name: error.error_message.clone(),
                value: None,
                caused_by: None,
            }
        }
    }
}

impl RpcError {
    /// Matches on the name of the RPC error (case-sensitive).
    ///
    /// Useful in `match` arm guards. A single trailing or leading asterisk (`'*'`) is allowed,
    /// and will instead check if the error name starts (or ends with) the input parameter.
    ///
    /// # Examples
    ///
    /// ```
    /// # let request_result = Result::<(), _>::Err(grammers_mtsender::RpcError {
    /// #     code: 400, name: "PHONE_CODE_INVALID".to_string(), value: None, caused_by: None });
    /// #
    /// match request_result {
    ///     Err(rpc_err) if rpc_err.is("SESSION_PASSWORD_NEEDED") => panic!(),
    ///     Err(rpc_err) if rpc_err.is("PHONE_CODE_*") => {},
    ///     _ => panic!()
    /// }
    /// ```
    pub fn is(&self, rpc_error: &str) -> bool {
        if let Some(rpc_error) = rpc_error.strip_suffix('*') {
            self.name.starts_with(rpc_error)
        } else if let Some(rpc_error) = rpc_error.strip_prefix('*') {
            self.name.ends_with(rpc_error)
        } else {
            self.name == rpc_error
        }
    }

    pub fn with_caused_by(mut self, constructor_id: u32) -> Self {
        self.caused_by = Some(constructor_id);
        self
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
    Rpc(RpcError),

    /// The request was cancelled or dropped, and the results won't arrive.
    Dropped,

    /// The error occured while reading the response.
    Read(ReadError),
}

impl std::error::Error for InvocationError {}

impl fmt::Display for InvocationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Rpc(err) => write!(f, "request error: {err}"),
            Self::Dropped => write!(f, "request error: dropped (cancelled)"),
            Self::Read(err) => write!(f, "request error: {err}"),
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
    /// #     grammers_mtsender::RpcError { code: 400, name: "PHONE_CODE_INVALID".to_string(), value: None, caused_by: None }));
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
            Self::Gen(err) => write!(f, "authorization error: {err}"),
            Self::Invoke(err) => write!(f, "authorization error: {err}"),
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
                value: None,
                caused_by: None,
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
                value: Some(31),
                caused_by: None,
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
                value: Some(2),
                caused_by: None,
            }
        );
    }
}
