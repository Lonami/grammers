// Copyright 2020 - developers of the `grammers` project.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// https://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or https://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.
use crate::sha1;
use std::fmt;

#[derive(Clone)]
pub struct AuthKey {
    pub(crate) data: [u8; 256],
    pub(crate) aux_hash: [u8; 8],
    pub(crate) key_id: [u8; 8],
}

impl PartialEq for AuthKey {
    fn eq(&self, other: &Self) -> bool {
        self.key_id == other.key_id
    }
}

/// Represents a Telegram's [authorization key].
///
/// To generate a new, valid authorization key, one should use the methods
/// provided by the [`generation`] module.
///
/// [authorization key]: https://core.telegram.org/mtproto/auth_key
/// [`generation`]: generation.html
impl AuthKey {
    /// Creates a new authorization key from the given binary data.
    pub fn from_bytes(data: [u8; 256]) -> Self {
        let sha = sha1!(&data);
        let aux_hash = {
            let mut buffer = [0; 8];
            buffer.copy_from_slice(&sha[0..8]);
            buffer
        };
        let key_id = {
            let mut buffer = [0; 8];
            buffer.copy_from_slice(&sha[12..12 + 8]);
            buffer
        };

        Self {
            data,
            aux_hash,
            key_id,
        }
    }

    /// Converts the authorization key to a sequence of bytes, which can
    /// be loaded back later.
    pub fn to_bytes(&self) -> [u8; 256] {
        self.data
    }

    /// Calculates the new nonce hash based on the current attributes.
    pub fn calc_new_nonce_hash(&self, new_nonce: &[u8; 32], number: u8) -> [u8; 16] {
        let data = {
            let mut buffer = Vec::with_capacity(new_nonce.len() + 1 + self.aux_hash.len());
            buffer.extend(new_nonce);
            buffer.push(number);
            buffer.extend(&self.aux_hash);
            buffer
        };

        let mut result = [0u8; 16];
        result.copy_from_slice(&sha1!(data)[4..]);
        result
    }
}

impl fmt::Debug for AuthKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("AuthKey")
            .field("key_id", &u64::from_le_bytes(self.key_id))
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn get_test_auth_key() -> AuthKey {
        let mut buffer = [0u8; 256];
        buffer
            .iter_mut()
            .enumerate()
            .for_each(|(i, x)| *x = i as u8);

        AuthKey::from_bytes(buffer)
    }

    fn get_test_new_nonce() -> [u8; 32] {
        let mut buffer = [0u8; 32];
        buffer
            .iter_mut()
            .enumerate()
            .for_each(|(i, x)| *x = i as u8);

        buffer
    }

    #[test]
    fn auth_key_aux_hash() {
        let auth_key = get_test_auth_key();
        let expected = [73, 22, 214, 189, 183, 247, 142, 104];

        assert_eq!(auth_key.aux_hash, expected);
    }

    #[test]
    fn auth_key_id() {
        let auth_key = get_test_auth_key();
        let expected = [50, 209, 88, 110, 164, 87, 223, 200];

        assert_eq!(auth_key.key_id, expected);
    }

    #[test]
    fn calc_new_nonce_hash1() {
        let auth_key = get_test_auth_key();
        let new_nonce = get_test_new_nonce();
        assert_eq!(
            auth_key.calc_new_nonce_hash(&new_nonce, 1),
            [194, 206, 210, 179, 62, 89, 58, 85, 210, 127, 74, 93, 171, 238, 124, 103]
        );
    }

    #[test]
    fn calc_new_nonce_hash2() {
        let auth_key = get_test_auth_key();
        let new_nonce = get_test_new_nonce();
        assert_eq!(
            auth_key.calc_new_nonce_hash(&new_nonce, 2),
            [244, 49, 142, 133, 189, 47, 243, 190, 132, 217, 254, 252, 227, 220, 227, 159]
        );
    }

    #[test]
    fn calc_new_nonce_hash3() {
        let auth_key = get_test_auth_key();
        let new_nonce = get_test_new_nonce();
        assert_eq!(
            auth_key.calc_new_nonce_hash(&new_nonce, 3),
            [75, 249, 215, 179, 125, 180, 19, 238, 67, 29, 40, 81, 118, 49, 203, 61]
        );
    }
}
