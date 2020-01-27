use getrandom::getrandom;
use openssl::aes::{aes_ige, AesKey};
use openssl::sha::sha1;
use openssl::symm::Mode;
use sha2::{Digest, Sha256};

pub enum Side {
    Client,
    Server,
}

pub enum Padding {
    Zero,
    Random,
}

// TODO these should be members of our AuthKey type (to be moved here)
pub fn auth_key_aux_hash(auth_key: &[u8; 256]) -> [u8; 8] {
    let mut buffer = [0; 8];
    buffer.copy_from_slice(&sha1(auth_key)[0..0 + 8]);
    buffer
}

pub fn auth_key_id(auth_key: &[u8; 256]) -> [u8; 8] {
    let mut buffer = [0; 8];
    buffer.copy_from_slice(&sha1(auth_key)[12..12 + 8]);
    buffer
}

/// This function implements the [MTProto 2.0 algorithm] for computing
/// `aes_key` and `aes_iv` from `auth_key` and `msg_key` as specified
///
/// [MTProto 2.0 algorithm]: https://core.telegram.org/mtproto/description#defining-aes-key-and-initialization-vector
pub fn encrypt_data_v2(
    plaintext: &[u8],
    auth_key: &[u8; 256],
    side: Side,
    padding: Padding,
) -> Vec<u8> {
    let key_id = auth_key_id(auth_key);

    // "Note that MTProto 2.0 requires from 12 to 1024 bytes of padding"
    // "[...] the resulting message length be divisible by 16 bytes"
    let random_padding = {
        let len = 16 + (16 - (plaintext.len() % 16));
        let mut buffer = vec![0; len];
        match padding {
            Padding::Zero => {}
            Padding::Random => {
                getrandom(&mut buffer).expect("failed to generate a secure padding");
            }
        }
        buffer
    };

    // "where x = 0 for messages from client to server and x = 8 for those from server to client."
    let x = match side {
        Side::Client => 0,
        Side::Server => 8,
    };

    // msg_key_large = SHA256 (substr (auth_key, 88+x, 32) + plaintext + random_padding);
    let msg_key_large = {
        let mut hasher = Sha256::new();
        hasher.input(&auth_key[88 + x..88 + x + 32]);
        hasher.input(&plaintext);
        hasher.input(&random_padding);
        hasher.result()
    };
    dbg!(&msg_key_large);

    // msg_key = substr (msg_key_large, 8, 16);
    let msg_key = { &msg_key_large[8..8 + 16] };
    dbg!(&msg_key);

    // sha256_a = SHA256 (msg_key + substr (auth_key, x, 36));
    let sha256_a = {
        let mut hasher = Sha256::new();
        hasher.input(msg_key);
        hasher.input(&auth_key[x..x + 36]);
        hasher.result()
    };

    // sha256_b = SHA256 (substr (auth_key, 40+x, 36) + msg_key);
    let sha256_b = {
        let mut hasher = Sha256::new();
        hasher.input(&auth_key[40 + x..40 + x + 36]);
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
    dbg!(&aes_key);

    // aes_iv = substr (sha256_b, 0, 8) + substr (sha256_a, 8, 16) + substr (sha256_b, 24, 8);
    let mut aes_iv = {
        let mut buffer = [0; 32];
        buffer[0..0 + 8].copy_from_slice(&sha256_b[0..0 + 8]);
        buffer[8..8 + 16].copy_from_slice(&sha256_a[8..8 + 16]);
        buffer[24..24 + 8].copy_from_slice(&sha256_b[24..24 + 8]);
        buffer
    };
    dbg!(&aes_iv);
    dbg!(&key_id);

    let mut padded = Vec::with_capacity(plaintext.len() + random_padding.len());
    padded.extend(plaintext);
    padded.extend(&random_padding);

    let ciphertext = {
        let mut buffer = vec![0; padded.len()];
        // Safe to unwrap because the key is of the correct length
        aes_ige(
            &padded,
            &mut buffer,
            &AesKey::new_encrypt(&aes_key).unwrap(),
            &mut aes_iv,
            Mode::Encrypt,
        );
        buffer
    };

    let mut result = Vec::with_capacity(key_id.len() + msg_key.len() + ciphertext.len());
    result.extend(&key_id);
    result.extend(msg_key);
    result.extend(&ciphertext);

    result
}
