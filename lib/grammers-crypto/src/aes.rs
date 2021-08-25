// Copyright 2020 - developers of the `grammers` project.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// https://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or https://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.
use aes::cipher::generic_array::GenericArray;
use aes::cipher::{BlockDecrypt, BlockEncrypt, NewBlockCipher};

/// Encrypt the input plaintext using the AES-IGE mode.
pub fn ige_encrypt(plaintext: &[u8], key: &[u8; 32], iv: &[u8; 32]) -> Vec<u8> {
    assert!(plaintext.len() % 16 == 0);
    let mut ciphertext = vec![0; plaintext.len()];

    let key = GenericArray::from_slice(key);
    let cipher = aes::Aes256::new(&key);

    let mut iv = *iv;
    let (iv1, iv2) = iv.split_at_mut(16);
    assert!(iv1.len() == 16);
    assert!(iv2.len() == 16);

    for (plaintext_block, ciphertext_block) in plaintext.chunks(16).zip(ciphertext.chunks_mut(16)) {
        assert!(plaintext_block.len() == 16);
        // block = block XOR iv1
        for i in 0..ciphertext_block.len() {
            ciphertext_block[i] = plaintext_block[i] ^ iv1[i];
        }
        
        // block = encrypt(block);
        let ciphertext_block = GenericArray::from_mut_slice(ciphertext_block);
        cipher.encrypt_block(ciphertext_block);

        // block = block XOR iv2
        for i in 0..ciphertext_block.len() {
            ciphertext_block[i] ^= iv2[i];
        }

        // save ciphertext and adjust iv
        iv1.copy_from_slice(ciphertext_block);
        iv2.copy_from_slice(plaintext_block);
    }

    ciphertext
}

/// Decrypt the input ciphertext using the AES-IGE mode.
pub fn ige_decrypt(ciphertext: &[u8], key: &[u8; 32], iv: &[u8; 32]) -> Vec<u8> {
    let size = ciphertext.len();
    assert!(size % 16 == 0);
    let mut plaintext = vec![0; size];

    let key = GenericArray::from_slice(key);
    let cipher = aes::Aes256::new(&key);
    let mut iv = *iv;

    for (ciphertext_block, plaintext_block) in ciphertext.chunks(16).zip(plaintext.chunks_mut(16)) {
        let plaintext_block = GenericArray::from_mut_slice(plaintext_block);
        // block = block XOR iv2
        for i in 0..16 {
            plaintext_block[i] = ciphertext_block[i] ^ iv[i + 16];
        }

        // block = decrypt(block);
        cipher.decrypt_block(plaintext_block);

        // block = block XOR iv1
        for i in 0..16 {
            plaintext_block[i] ^= iv[i];
        }

        // save plaintext and adjust iv
        iv[0..16].copy_from_slice(ciphertext_block);
        iv[16..32].copy_from_slice(plaintext_block);
    }

    plaintext
}
