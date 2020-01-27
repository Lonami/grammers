//! Contains the steps required to generate an authorization key.
use getrandom::getrandom;
use grammers_tl_types::{self as tl};
use std::error::Error;
use std::fmt;
use std::io;

/// Represents an error that occured during the generation of an
/// authorization key.
#[derive(Debug)]
pub enum AuthKeyGenError {
    /// The server's nonce did not match ours.
    BadNonce {
        client_nonce: [u8; 16],
        server_nonce: [u8; 16],
    },

    /// The server's PQ number was not of the right size.
    WrongSizePQ { size: usize, expected: usize },
}

impl Error for AuthKeyGenError {}

impl fmt::Display for AuthKeyGenError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // TODO better display
        write!(f, "{:?}", self)
    }
}

impl From<AuthKeyGenError> for io::Error {
    fn from(error: AuthKeyGenError) -> Self {
        io::Error::new(io::ErrorKind::InvalidData, error)
    }
}

/// Step 1. Generates a secure random nonce.
pub fn generate_nonce() -> [u8; 16] {
    let mut buffer = [0; 16];
    getrandom(&mut buffer).expect("failed to generate a secure nonce");
    buffer
}

/// Step 2. Validate the PQ response. Return `(p, q)` if it's valid.
pub fn validate_pq(
    client_nonce: &[u8; 16],
    res_pq: &tl::types::ResPQ,
) -> Result<u64, AuthKeyGenError> {
    if *client_nonce != res_pq.nonce {
        return Err(AuthKeyGenError::BadNonce {
            client_nonce: client_nonce.clone(),
            server_nonce: res_pq.nonce.clone(),
        });
    }

    if res_pq.pq.len() != 8 {
        return Err(AuthKeyGenError::WrongSizePQ {
            size: res_pq.pq.len(),
            expected: 8,
        });
    }

    let pq = {
        let mut buffer = [0; 8];
        buffer.copy_from_slice(&res_pq.pq);
        u64::from_be_bytes(buffer)
    };

    Ok(pq)
}
