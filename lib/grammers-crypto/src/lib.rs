// Copyright 2020 - developers of the `grammers` project.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// https://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or https://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.
pub mod aes;
mod auth_key;
pub mod factorize;
pub mod hex;
pub mod ring_buffer;
pub mod rsa;
pub mod sha;
pub mod two_factor_auth;

pub use auth_key::AuthKey;
use getrandom::getrandom;
pub use ring_buffer::RingBuffer;
use std::fmt;

#[derive(Clone, Debug, PartialEq)]
pub enum Error {
    /// The ciphertext is either too small or not padded correctly.
    InvalidBuffer,

    /// The server replied with the ID of a different authorization key.
    AuthKeyMismatch,

    /// The key of the message did not match our expectations.
    MessageKeyMismatch,
}

impl std::error::Error for Error {}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match *self {
            Error::InvalidBuffer => write!(f, "invalid ciphertext buffer length"),
            Error::AuthKeyMismatch => write!(f, "server authkey mismatches with ours"),
            Error::MessageKeyMismatch => write!(f, "server msgkey mismatches with ours"),
        }
    }
}

enum Side {
    Client,
    Server,
}

impl Side {
    // "where x = 0 for messages from client to server and x = 8 for those from server to client."
    fn x(&self) -> usize {
        match *self {
            Side::Client => 0,
            Side::Server => 8,
        }
    }
}

/// Calculate the key based on Telegram [guidelines for MTProto 2],
/// returning the pair `(key, iv)` for use in AES-IGE mode.
///
/// [guidelines for MTProto 2]: https://core.telegram.org/mtproto/description#defining-aes-key-and-initialization-vector
fn calc_key(auth_key: &AuthKey, msg_key: &[u8; 16], side: Side) -> ([u8; 32], [u8; 32]) {
    let x = side.x();

    // sha256_a = SHA256 (msg_key + substr (auth_key, x, 36));
    let sha256_a = sha256!(msg_key, &auth_key.data[x..x + 36]);

    // sha256_b = SHA256 (substr (auth_key, 40+x, 36) + msg_key);
    let sha256_b = sha256!(&auth_key.data[40 + x..40 + x + 36], msg_key);

    // aes_key = substr (sha256_a, 0, 8) + substr (sha256_b, 8, 16) + substr (sha256_a, 24, 8);
    let aes_key = {
        let mut buffer = [0; 32];
        buffer[0..8].copy_from_slice(&sha256_a[0..8]);
        buffer[8..8 + 16].copy_from_slice(&sha256_b[8..8 + 16]);
        buffer[24..24 + 8].copy_from_slice(&sha256_a[24..24 + 8]);
        buffer
    };

    // aes_iv = substr (sha256_b, 0, 8) + substr (sha256_a, 8, 16) + substr (sha256_b, 24, 8);
    let aes_iv = {
        let mut buffer = [0; 32];
        buffer[0..8].copy_from_slice(&sha256_b[0..8]);
        buffer[8..8 + 16].copy_from_slice(&sha256_a[8..8 + 16]);
        buffer[24..24 + 8].copy_from_slice(&sha256_b[24..24 + 8]);
        buffer
    };

    (aes_key, aes_iv)
}

/// Determines the padding length needed for a plaintext of a certain length,
/// according to the following citation:
///
/// > Note that MTProto 2.0 requires from 12 to 1024 bytes of padding
/// > [...] the resulting message length be divisible by 16 bytes
fn determine_padding_v2_length(len: usize) -> usize {
    16 + (16 - (len % 16))
}

// Inner body of `encrypt_data_v2`, separated for testing purposes.
fn do_encrypt_data_v2(buffer: &mut RingBuffer<u8>, auth_key: &AuthKey, random_padding: &[u8; 32]) {
    // "Note that MTProto 2.0 requires from 12 to 1024 bytes of padding"
    // "[...] the resulting message length be divisible by 16 bytes"
    let padding_len = determine_padding_v2_length(buffer.len());
    buffer.extend(random_padding.iter().take(padding_len));

    // Encryption is done by the client
    let side = Side::Client;
    let x = side.x();

    // msg_key_large = SHA256 (substr (auth_key, 88+x, 32) + plaintext + random_padding);
    let msg_key_large = sha256!(&auth_key.data[88 + x..88 + x + 32], &buffer[..]);

    // msg_key = substr (msg_key_large, 8, 16);
    let msg_key = {
        let mut buffer = [0; 16];
        buffer.copy_from_slice(&msg_key_large[8..8 + 16]);
        buffer
    };

    // Calculate the key
    let (key, iv) = calc_key(auth_key, &msg_key, side);

    aes::ige_encrypt(&mut buffer[..], &key, &iv);

    let mut head = buffer.shift(auth_key.key_id.len() + msg_key.len());
    head.extend(auth_key.key_id.iter().copied());
    head.extend(msg_key.iter().copied());
}

/// This function implements the [MTProto 2.0 algorithm] for computing
/// `aes_key` and `aes_iv` from `auth_key` and `msg_key` as specified
///
/// [MTProto 2.0 algorithm]: https://core.telegram.org/mtproto/description#defining-aes-key-and-initialization-vector
pub fn encrypt_data_v2(buffer: &mut RingBuffer<u8>, auth_key: &AuthKey) {
    let random_padding = {
        let mut rnd = [0; 32];
        getrandom(&mut rnd).expect("failed to generate a secure padding");
        rnd
    };

    do_encrypt_data_v2(buffer, auth_key, &random_padding)
}

/// This method is the inverse of `encrypt_data_v2`.
pub fn decrypt_data_v2(ciphertext: &[u8], auth_key: &AuthKey) -> Result<Vec<u8>, Error> {
    // Decryption is done from the server
    let side = Side::Server;
    let x = side.x();

    if ciphertext.len() < 24 || (ciphertext.len() - 24) % 16 != 0 {
        return Err(Error::InvalidBuffer);
    }

    // TODO Check salt, session_id and sequence_number
    let key_id = &ciphertext[..8];
    if auth_key.key_id != *key_id {
        return Err(Error::AuthKeyMismatch);
    }

    let msg_key = {
        let mut buffer = [0; 16];
        buffer.copy_from_slice(&ciphertext[8..8 + 16]);
        buffer
    };

    let (key, iv) = calc_key(auth_key, &msg_key, Side::Server);
    let plaintext = decrypt_ige(&ciphertext[24..], &key, &iv);

    // https://core.telegram.org/mtproto/security_guidelines#mtproto-encrypted-messages
    let our_key = sha256!(&auth_key.data[88 + x..88 + x + 32], &plaintext);

    if msg_key != our_key[8..8 + 16] {
        return Err(Error::MessageKeyMismatch);
    }

    Ok(plaintext)
}

/// Generate the AES key and initialization vector from the server nonce
/// and the new client nonce. This is done after the DH exchange.
pub fn generate_key_data_from_nonce(
    server_nonce: &[u8; 16],
    new_nonce: &[u8; 32],
) -> ([u8; 32], [u8; 32]) {
    let hash1 = sha1!(new_nonce, server_nonce);
    let hash2 = sha1!(server_nonce, new_nonce);
    let hash3 = sha1!(new_nonce, new_nonce);

    // key = hash1 + hash2[:12]
    let key = {
        let mut buffer = [0; 32];
        buffer[..hash1.len()].copy_from_slice(&hash1);
        buffer[hash1.len()..].copy_from_slice(&hash2[..12]);
        buffer
    };

    // iv = hash2[12:20] + hash3 + new_nonce[:4]
    let iv = {
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
    let mut padded = if plaintext.len() % 16 == 0 {
        plaintext.to_vec()
    } else {
        let pad_len = (16 - (plaintext.len() % 16)) % 16;
        let mut padded = Vec::with_capacity(plaintext.len() + pad_len);
        padded.extend(plaintext);

        let mut buffer = vec![0; pad_len];
        getrandom(&mut buffer).expect("failed to generate random padding for encryption");
        padded.extend(&buffer);
        padded
    };

    aes::ige_encrypt(padded.as_mut(), key, iv);
    padded
}

/// Decrypt data using AES-IGE. Panics if the plaintext is not padded
/// to 16 bytes.
pub fn decrypt_ige(padded_ciphertext: &[u8], key: &[u8; 32], iv: &[u8; 32]) -> Vec<u8> {
    aes::ige_decrypt(padded_ciphertext, key, iv)
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

    fn get_test_msg_key() -> [u8; 16] {
        let mut buffer = [0u8; 16];
        buffer
            .iter_mut()
            .enumerate()
            .for_each(|(i, x)| *x = i as u8);

        buffer
    }

    fn get_test_aes_key_or_iv() -> [u8; 32] {
        let mut buffer = [0u8; 32];
        buffer
            .iter_mut()
            .enumerate()
            .for_each(|(i, x)| *x = i as u8);

        buffer
    }

    #[test]
    fn calc_client_key() {
        let auth_key = get_test_auth_key();
        let msg_key = get_test_msg_key();
        let expected = (
            [
                112, 78, 208, 156, 139, 65, 102, 138, 232, 249, 157, 36, 71, 56, 247, 29, 189, 220,
                68, 70, 155, 107, 189, 74, 168, 87, 61, 208, 66, 189, 5, 158,
            ],
            [
                77, 38, 96, 0, 165, 80, 237, 171, 191, 76, 124, 228, 15, 208, 4, 60, 201, 34, 48,
                24, 76, 211, 23, 165, 204, 156, 36, 130, 253, 59, 147, 24,
            ],
        );
        assert_eq!(calc_key(&auth_key, &msg_key, Side::Client), expected);
    }

    #[test]
    fn calc_server_key() {
        let auth_key = get_test_auth_key();
        let msg_key = get_test_msg_key();
        let expected = (
            [
                33, 119, 37, 121, 155, 36, 88, 6, 69, 129, 116, 161, 252, 251, 200, 131, 144, 104,
                7, 177, 80, 51, 253, 208, 234, 43, 77, 105, 207, 156, 54, 78,
            ],
            [
                102, 154, 101, 56, 145, 122, 79, 165, 108, 163, 35, 96, 164, 49, 201, 22, 11, 228,
                173, 136, 113, 64, 152, 13, 171, 145, 206, 123, 220, 71, 255, 188,
            ],
        );
        assert_eq!(calc_key(&auth_key, &msg_key, Side::Server), expected);
    }

    #[test]
    fn encrypt_client_data_v2() {
        let mut buffer = RingBuffer::with_capacity(0, 0);
        buffer.extend(b"Hello, world! This data should remain secure!");
        let auth_key = get_test_auth_key();
        let random_padding = [0; 32];
        let expected = vec![
            50, 209, 88, 110, 164, 87, 223, 200, 168, 23, 41, 212, 109, 181, 64, 25, 162, 191, 215,
            247, 68, 249, 185, 108, 79, 113, 108, 253, 196, 71, 125, 178, 162, 193, 95, 109, 219,
            133, 35, 95, 185, 85, 47, 29, 132, 7, 198, 170, 234, 0, 204, 132, 76, 90, 27, 246, 172,
            68, 183, 155, 94, 220, 42, 35, 134, 139, 61, 96, 115, 165, 144, 153, 44, 15, 41, 117,
            36, 61, 86, 62, 161, 128, 210, 24, 238, 117, 124, 154,
        ];

        do_encrypt_data_v2(&mut buffer, &auth_key, &random_padding);
        assert_eq!(&buffer[..], expected);
    }

    #[test]
    fn decrypt_server_data_v2() {
        let ciphertext = vec![
            122, 113, 131, 194, 193, 14, 79, 77, 249, 69, 250, 154, 154, 189, 53, 231, 195, 132,
            11, 97, 240, 69, 48, 79, 57, 103, 76, 25, 192, 226, 9, 120, 79, 80, 246, 34, 106, 7,
            53, 41, 214, 117, 201, 44, 191, 11, 250, 140, 153, 167, 155, 63, 57, 199, 42, 93, 154,
            2, 109, 67, 26, 183, 64, 124, 160, 78, 204, 85, 24, 125, 108, 69, 241, 120, 113, 82,
            78, 221, 144, 206, 160, 46, 215, 40, 225, 77, 124, 177, 138, 234, 42, 99, 97, 88, 240,
            148, 89, 169, 67, 119, 16, 216, 148, 199, 159, 54, 140, 78, 129, 100, 183, 100, 126,
            169, 134, 18, 174, 254, 148, 44, 93, 146, 18, 26, 203, 141, 176, 45, 204, 206, 182,
            109, 15, 135, 32, 172, 18, 160, 109, 176, 88, 43, 253, 149, 91, 227, 79, 54, 81, 24,
            227, 186, 184, 205, 8, 12, 230, 180, 91, 40, 234, 197, 109, 205, 42, 41, 55, 78,
        ];
        let auth_key = AuthKey::from_bytes([
            93, 46, 125, 101, 244, 158, 194, 139, 208, 41, 168, 135, 97, 234, 39, 184, 164, 199,
            159, 18, 34, 101, 37, 68, 62, 125, 124, 89, 110, 243, 48, 53, 48, 219, 33, 7, 232, 154,
            169, 151, 199, 160, 22, 74, 182, 148, 24, 122, 222, 255, 21, 107, 214, 239, 113, 24,
            161, 150, 35, 71, 117, 60, 14, 126, 137, 160, 53, 75, 142, 195, 100, 249, 153, 126,
            113, 188, 105, 35, 251, 134, 232, 228, 52, 145, 224, 16, 96, 106, 108, 232, 69, 226,
            250, 1, 148, 9, 119, 239, 10, 163, 42, 223, 90, 151, 219, 246, 212, 40, 236, 4, 52,
            215, 23, 162, 211, 173, 25, 98, 44, 192, 88, 135, 100, 33, 19, 199, 150, 95, 251, 134,
            42, 62, 60, 203, 10, 185, 90, 221, 218, 87, 248, 146, 69, 219, 215, 107, 73, 35, 72,
            248, 233, 75, 213, 167, 192, 224, 184, 72, 8, 82, 60, 253, 30, 168, 11, 50, 254, 154,
            209, 152, 188, 46, 16, 63, 206, 183, 213, 36, 146, 236, 192, 39, 58, 40, 103, 75, 201,
            35, 238, 229, 146, 101, 171, 23, 160, 2, 223, 31, 74, 162, 197, 155, 129, 154, 94, 94,
            29, 16, 94, 193, 23, 51, 111, 92, 118, 198, 177, 135, 3, 125, 75, 66, 112, 206, 233,
            204, 33, 7, 29, 151, 233, 188, 162, 32, 198, 215, 176, 27, 153, 140, 242, 229, 205,
            185, 165, 14, 205, 161, 133, 42, 54, 230, 53, 105, 12, 142,
        ]);
        let expected = vec![
            252, 130, 106, 2, 36, 139, 40, 253, 96, 242, 196, 130, 36, 67, 173, 104, 1, 240, 193,
            194, 145, 139, 48, 94, 2, 0, 0, 0, 88, 0, 0, 0, 220, 248, 241, 115, 2, 0, 0, 0, 1, 168,
            193, 194, 145, 139, 48, 94, 1, 0, 0, 0, 28, 0, 0, 0, 8, 9, 194, 158, 196, 253, 51, 173,
            145, 139, 48, 94, 24, 168, 142, 166, 7, 238, 88, 22, 252, 130, 106, 2, 36, 139, 40,
            253, 1, 204, 193, 194, 145, 139, 48, 94, 2, 0, 0, 0, 20, 0, 0, 0, 197, 115, 119, 52,
            196, 253, 51, 173, 145, 139, 48, 94, 100, 8, 48, 0, 0, 0, 0, 0, 252, 230, 103, 4, 163,
            205, 142, 233, 208, 174, 111, 171, 103, 44, 96, 192, 74, 63, 31, 212, 73, 14, 81, 246,
        ];

        assert_eq!(decrypt_data_v2(&ciphertext, &auth_key).unwrap(), expected);
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

    #[test]
    fn verify_ige_encryption() {
        let plaintext = get_test_aes_key_or_iv(); // Encrypting the key with itself
        let key = get_test_aes_key_or_iv();
        let iv = get_test_aes_key_or_iv();
        let expected = vec![
            226, 129, 18, 165, 62, 92, 137, 199, 177, 234, 128, 113, 193, 51, 105, 159, 212, 232,
            107, 38, 196, 186, 201, 252, 90, 241, 171, 140, 226, 122, 68, 164,
        ];
        assert_eq!(encrypt_ige(&plaintext, &key, &iv), expected);
    }

    #[test]
    fn verify_ige_decryption() {
        let ciphertext = get_test_aes_key_or_iv(); // Decrypting the key with itself
        let key = get_test_aes_key_or_iv();
        let iv = get_test_aes_key_or_iv();
        let expected = vec![
            229, 119, 122, 250, 205, 123, 44, 22, 247, 172, 64, 202, 230, 30, 246, 3, 254, 230, 9,
            143, 184, 168, 134, 10, 185, 238, 103, 44, 215, 229, 186, 204,
        ];
        assert_eq!(decrypt_ige(&ciphertext, &key, &iv), expected);
    }
}
