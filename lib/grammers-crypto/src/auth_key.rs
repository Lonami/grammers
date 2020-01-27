use openssl::sha::sha1;
use std::fmt;

#[derive(Clone)]
pub struct AuthKey {
    pub(crate) data: [u8; 256],
    pub(crate) aux_hash: [u8; 8],
    pub(crate) key_id: [u8; 8],
}

/// Represents a Telegram's [authorization key].
///
/// [authorization key]: https://core.telegram.org/mtproto/auth_key
impl AuthKey {
    /// Creates a new authorization key from the given binary data.
    pub fn from_bytes(data: [u8; 256]) -> Self {
        let sha = sha1(&data);
        let aux_hash = {
            let mut buffer = [0; 8];
            buffer.copy_from_slice(&sha[0..0 + 8]);
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
}
