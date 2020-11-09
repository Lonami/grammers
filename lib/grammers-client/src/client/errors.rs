use std::error::Error;
use std::fmt;
use crate::InvocationError;

/// This error occurs when finding "reply to message" fails for a message
#[derive(Debug)]
pub enum GetReplyToMessageError {
    // Given message does not have reply to message
    NotFound,

    // Failed to get the reply message due to InvocationError
    Invocation(InvocationError)
}

impl Error for GetReplyToMessageError {}

impl fmt::Display for GetReplyToMessageError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotFound => write!(f, "get_reply_to_message error, Given message does not have reply to message"),
            Self::Invocation(e) => write!(f, "get_reply_to_message error, bad invoke: {}", e),
        }
    }
}

impl From<InvocationError> for GetReplyToMessageError {
    fn from(e: InvocationError) -> Self {
        return Self::Invocation(e)
    }
}
