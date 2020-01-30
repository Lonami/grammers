use std::io;

use grammers_tl_types::Deserializable;

use crate::errors::DeserializeError;

/// Checks a message buffer for common errors
pub(crate) fn check_message_buffer(message: &[u8]) -> io::Result<()> {
    if message.len() == 4 {
        // Probably a negative HTTP error code
        Err(io::Error::new(
            io::ErrorKind::InvalidData,
            DeserializeError::TransportError {
                // Safe to unwrap because we just checked the length
                code: i32::from_bytes(message).unwrap(),
            },
        ))
    } else if message.len() < 20 {
        Err(io::Error::new(
            io::ErrorKind::InvalidData,
            DeserializeError::MessageBufferTooSmall,
        ))
    } else {
        Ok(())
    }
}
