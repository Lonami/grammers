mod auth_key;
pub use auth_key::AuthKey;
use getrandom::getrandom;
use openssl::aes::{aes_ige, AesKey};
use openssl::sha::sha1;
use openssl::symm::Mode;
use sha2::{Digest, Sha256};

pub enum Side {
    Client,
    Server,
}

// Inner body of `encrypt_data_v2`, separated for testing purposes.
fn encrypt_padded_data_v2(padded_plaintext: &[u8], auth_key: &AuthKey, side: Side) -> Vec<u8> {
    // "where x = 0 for messages from client to server and x = 8 for those from server to client."
    let x = match side {
        Side::Client => 0,
        Side::Server => 8,
    };

    // msg_key_large = SHA256 (substr (auth_key, 88+x, 32) + plaintext + random_padding);
    let msg_key_large = {
        let mut hasher = Sha256::new();
        hasher.input(&auth_key.data[88 + x..88 + x + 32]);
        hasher.input(&padded_plaintext);
        hasher.result()
    };

    // msg_key = substr (msg_key_large, 8, 16);
    let msg_key = { &msg_key_large[8..8 + 16] };

    // sha256_a = SHA256 (msg_key + substr (auth_key, x, 36));
    let sha256_a = {
        let mut hasher = Sha256::new();
        hasher.input(msg_key);
        hasher.input(&auth_key.data[x..x + 36]);
        hasher.result()
    };

    // sha256_b = SHA256 (substr (auth_key, 40+x, 36) + msg_key);
    let sha256_b = {
        let mut hasher = Sha256::new();
        hasher.input(&auth_key.data[40 + x..40 + x + 36]);
        hasher.input(msg_key);
        hasher.result()
    };

    // aes_key = substr (sha256_a, 0, 8) + substr (sha256_b, 8, 16) + substr (sha256_a, 24, 8);
    let aes_key = {
        let mut buffer = [0; 32];
        buffer[0..0 + 8].copy_from_slice(&sha256_a[0..0 + 8]);
        buffer[8..8 + 16].copy_from_slice(&sha256_b[8..8 + 16]);
        buffer[24..24 + 8].copy_from_slice(&sha256_a[24..24 + 8]);
        buffer
    };

    // aes_iv = substr (sha256_b, 0, 8) + substr (sha256_a, 8, 16) + substr (sha256_b, 24, 8);
    let mut aes_iv = {
        let mut buffer = [0; 32];
        buffer[0..0 + 8].copy_from_slice(&sha256_b[0..0 + 8]);
        buffer[8..8 + 16].copy_from_slice(&sha256_a[8..8 + 16]);
        buffer[24..24 + 8].copy_from_slice(&sha256_b[24..24 + 8]);
        buffer
    };

    let ciphertext = {
        let mut buffer = vec![0; padded_plaintext.len()];
        // Safe to unwrap because the key is of the correct length
        aes_ige(
            &padded_plaintext,
            &mut buffer,
            &AesKey::new_encrypt(&aes_key).unwrap(),
            &mut aes_iv,
            Mode::Encrypt,
        );
        buffer
    };

    let mut result = Vec::with_capacity(auth_key.key_id.len() + msg_key.len() + ciphertext.len());
    result.extend(&auth_key.key_id);
    result.extend(msg_key);
    result.extend(&ciphertext);

    result
}

/// Determines the padding length needed for a plaintext of a certain length,
/// according to the following citation:
///
/// > Note that MTProto 2.0 requires from 12 to 1024 bytes of padding
/// > [...] the resulting message length be divisible by 16 bytes
fn determine_padding_v2_length(len: usize) -> usize {
    16 + (16 - (len % 16))
}

/// This function implements the [MTProto 2.0 algorithm] for computing
/// `aes_key` and `aes_iv` from `auth_key` and `msg_key` as specified
///
/// [MTProto 2.0 algorithm]: https://core.telegram.org/mtproto/description#defining-aes-key-and-initialization-vector
pub fn encrypt_data_v2(plaintext: &[u8], auth_key: &AuthKey, side: Side) -> Vec<u8> {
    // "Note that MTProto 2.0 requires from 12 to 1024 bytes of padding"
    // "[...] the resulting message length be divisible by 16 bytes"
    let random_padding = {
        let mut buffer = vec![0; determine_padding_v2_length(plaintext.len())];
        getrandom(&mut buffer).expect("failed to generate a secure padding");
        buffer
    };

    let mut padded = Vec::with_capacity(plaintext.len() + random_padding.len());
    padded.extend(plaintext);
    padded.extend(&random_padding);
    encrypt_padded_data_v2(&padded, auth_key, side)
}

/// Generate the AES key and initialization vector from the server nonce
/// and the new client nonce. This is done after the DH exchange.
pub fn generate_key_data_from_nonce(
    server_nonce: &[u8; 16],
    new_nonce: &[u8; 32],
) -> ([u8; 32], [u8; 32]) {
    // hash1 = sha1(new_nonce + server_nonce).digest()
    let hash1: [u8; 20] = {
        let mut buffer = Vec::with_capacity(new_nonce.len() + server_nonce.len());
        buffer.extend(new_nonce);
        buffer.extend(server_nonce);
        sha1(&buffer)
    };
    // hash2 = sha1(server_nonce + new_nonce).digest()
    let hash2: [u8; 20] = {
        let mut buffer = Vec::with_capacity(server_nonce.len() + new_nonce.len());
        buffer.extend(server_nonce);
        buffer.extend(new_nonce);
        sha1(&buffer)
    };
    // hash3 = sha1(new_nonce + new_nonce).digest()
    let hash3: [u8; 20] = {
        let mut buffer = Vec::with_capacity(new_nonce.len() + new_nonce.len());
        buffer.extend(new_nonce);
        buffer.extend(new_nonce);
        sha1(&buffer)
    };

    // key = hash1 + hash2[:12]
    let key: [u8; 32] = {
        let mut buffer = [0; 32];
        buffer[..hash1.len()].copy_from_slice(&hash1);
        buffer[hash1.len()..].copy_from_slice(&hash2[..12]);
        buffer
    };

    // iv = hash2[12:20] + hash3 + new_nonce[:4]
    let iv: [u8; 32] = {
        let mut buffer = [0; 32];
        buffer[..8].copy_from_slice(&hash2[12..]);
        buffer[8..28].copy_from_slice(&hash3);
        buffer[28..].copy_from_slice(&new_nonce[..4]);
        buffer
    };

    (key, iv)
}

/// Encrypt data using AES-IGE.
pub fn encrypt_ige(plaintext: &[u8], key: &[u8; 32], iv: &[u8; 32]) -> Vec<u8> {
    let mut padded: Vec<u8>;
    let padded_plaintext = if plaintext.len() % 16 == 0 {
        plaintext
    } else {
        let pad_len = (16 - (plaintext.len() % 16)) % 16;
        padded = Vec::with_capacity(plaintext.len() + pad_len);
        padded.extend(plaintext);

        let mut buffer = vec![0; pad_len];
        getrandom(&mut buffer).expect("failed to generate random padding for encryption");
        padded.extend(&buffer);
        eprintln!("had to pad now have len {}", padded.len());

        &padded
    };

    let mut buffer = vec![0; padded_plaintext.len()];
    // Safe to unwrap because the key is of the correct length
    aes_ige(
        padded_plaintext,
        &mut buffer,
        &AesKey::new_encrypt(key).unwrap(),
        &mut iv.clone(),
        Mode::Encrypt,
    );
    buffer
}

/// Decrypt data using AES-IGE. Panics if the plaintext is not padded
/// to 16 bytes.
pub fn decrypt_ige(padded_ciphertext: &[u8], key: &[u8; 32], iv: &[u8; 32]) -> Vec<u8> {
    let mut buffer = vec![0; padded_ciphertext.len()];
    // Safe to unwrap because the key is of the correct length
    aes_ige(
        &padded_ciphertext,
        &mut buffer,
        &AesKey::new_decrypt(key).unwrap(),
        &mut iv.clone(),
        Mode::Decrypt,
    );
    buffer
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
    fn encrypt_client_data_v2() {
        let padded_plaintext = {
            let mut buffer = Vec::new();
            buffer.extend(b"Hello, world! This data should remain secure!".iter());
            for _ in 0..determine_padding_v2_length(buffer.len()) {
                buffer.push(0);
            }

            buffer
        };
        let auth_key = get_test_auth_key();
        let side = Side::Client;
        let expected = vec![
            50, 209, 88, 110, 164, 87, 223, 200, 168, 23, 41, 212, 109, 181, 64, 25, 162, 191, 215,
            247, 68, 249, 185, 108, 79, 113, 108, 253, 196, 71, 125, 178, 162, 193, 95, 109, 219,
            133, 35, 95, 185, 85, 47, 29, 132, 7, 198, 170, 234, 0, 204, 132, 76, 90, 27, 246, 172,
            68, 183, 155, 94, 220, 42, 35, 134, 139, 61, 96, 115, 165, 144, 153, 44, 15, 41, 117,
            36, 61, 86, 62, 161, 128, 210, 24, 238, 117, 124, 154,
        ];

        assert_eq!(
            encrypt_padded_data_v2(&padded_plaintext, &auth_key, side),
            expected
        );
    }

    #[test]
    fn key_from_nonce() {
        let server_nonce = {
            let mut buffer = [0u8; 16];
            buffer
                .iter_mut()
                .enumerate()
                .for_each(|(i, x)| *x = i as u8);
            buffer
        };
        let new_nonce = {
            let mut buffer = [0u8; 32];
            buffer
                .iter_mut()
                .enumerate()
                .for_each(|(i, x)| *x = i as u8);
            buffer
        };

        let (key, iv) = generate_key_data_from_nonce(&server_nonce, &new_nonce);
        assert_eq!(
            key,
            [
                7, 88, 241, 83, 59, 97, 93, 36, 246, 232, 169, 74, 111, 203, 238, 10, 85, 234, 171,
                34, 23, 215, 41, 92, 169, 33, 61, 26, 45, 125, 22, 166
            ]
        );
        assert_eq!(
            iv,
            [
                90, 132, 16, 142, 152, 5, 101, 108, 232, 100, 7, 14, 22, 110, 98, 24, 246, 120, 62,
                133, 17, 71, 26, 90, 183, 128, 44, 242, 0, 1, 2, 3
            ]
        );
    }
}
