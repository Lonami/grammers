//! Contains the steps required to generate an authorization key.
use getrandom::getrandom;

/// Step 1. Generates a secure random nonce.
pub fn generate_nonce() -> [u8; 16] {
    let mut buffer = [0; 16];
    getrandom(&mut buffer).expect("failed to generate a secure nonce");
    buffer
}

// Step 2. Validate the PQ response.
pub fn validate_pq() {
    unimplemented!();
}
