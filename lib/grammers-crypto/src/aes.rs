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
    let mut ciphertext = Vec::with_capacity(plaintext.len());

    let key = GenericArray::from_slice(key);
    let cipher = aes::Aes256::new(&key);

    let mut iv = iv.clone();
    let (iv1, iv2) = iv.split_at_mut(16);

    for plaintext_block in plaintext.chunks(16) {
        // block = block XOR iv1
        let mut xored_block = [0; 16];
        xored_block
            .iter_mut()
            .zip(plaintext_block.iter().zip(iv1.iter()))
            .for_each(|(x, (a, b))| *x = a ^ b);

        // block = encrypt(block);
        let ciphertext_block = GenericArray::from_mut_slice(&mut xored_block);
        cipher.encrypt_block(ciphertext_block);

        // block = block XOR iv2
        ciphertext_block
            .iter_mut()
            .zip(iv2.iter())
            .for_each(|(x, a)| *x ^= a);

        // save ciphertext and adjust iv
        ciphertext.extend(ciphertext_block.iter());
        iv1.copy_from_slice(ciphertext_block);
        iv2.copy_from_slice(plaintext_block);
    }

    ciphertext
}

/// Decrypt the input ciphertext using the AES-IGE mode.
pub fn ige_decrypt(ciphertext: &[u8], key: &[u8; 32], iv: &[u8; 32]) -> Vec<u8> {
    assert!(ciphertext.len() % 16 == 0);
    let mut plaintext = Vec::with_capacity(ciphertext.len());

    let key = GenericArray::from_slice(key);
    let cipher = aes::Aes256::new(&key);

    let mut iv = iv.clone();
    let (iv1, iv2) = iv.split_at_mut(16);

    for ciphertext_block in ciphertext.chunks(16) {
        // block = block XOR iv2
        let mut xored_block = [0; 16];
        xored_block
            .iter_mut()
            .zip(ciphertext_block.iter().zip(iv2.iter()))
            .for_each(|(x, (a, b))| *x = a ^ b);

        // block = decrypt(block);
        let plaintext_block = GenericArray::from_mut_slice(&mut xored_block);
        cipher.decrypt_block(plaintext_block);

        // block = block XOR iv1
        plaintext_block
            .iter_mut()
            .zip(iv1.iter())
            .for_each(|(x, a)| *x ^= a);

        // save plaintext and adjust iv
        plaintext.extend(plaintext_block.iter());
        iv1.clone_from_slice(ciphertext_block);
        iv2.clone_from_slice(plaintext_block);
    }

    plaintext
}
