// Copyright 2020 - developers of the `grammers` project.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// https://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or https://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

//! [AES-Infinite Garble Extension](https://mgp25.com/blog/2015/06/21/AESIGE/) implementation.

#![allow(deprecated)] // see https://github.com/RustCrypto/block-ciphers/issues/509

use std::mem;

use aes::Aes256;
use aes::cipher::generic_array::GenericArray;
use aes::cipher::{BlockDecrypt, BlockEncrypt, KeyInit};

/// Encrypt the input plaintext in-place using the AES-IGE mode.
/// The buffer length must be a multiple of 16.
pub fn ige_encrypt(buffer: &mut [u8], key: &[u8; 32], iv: &[u8; 32]) {
    assert_eq!(buffer.len() % 16, 0);

    let cipher = Aes256::new(GenericArray::from_slice(key));

    let mut iv1: [u8; 16] = iv[0..16].try_into().unwrap();
    let mut iv2: [u8; 16] = iv[16..32].try_into().unwrap();

    let mut next_iv2 = [0u8; 16];

    for block in buffer.chunks_mut(16) {
        // next iv2 = block (plaintext)
        next_iv2.copy_from_slice(block);

        // block (plaintext) XOR iv1 (previous ciphertext)
        for i in 0..16 {
            block[i] ^= iv1[i]
        }

        cipher.encrypt_block(GenericArray::from_mut_slice(block));

        // block (ciphertext) XOR iv2 (previous plaintext)
        for i in 0..16 {
            block[i] ^= iv2[i]
        }

        // iv1 = block (ciphertext)
        iv1.copy_from_slice(block);

        // iv2 = next iv2 (plaintext)
        mem::swap(&mut iv2, &mut next_iv2);
    }
}

/// Decrypt the input ciphertext in-place using the AES-IGE mode.
/// The buffer length must be a multiple of 16.
pub fn ige_decrypt(buffer: &mut [u8], key: &[u8; 32], iv: &[u8; 32]) {
    assert_eq!(buffer.len() % 16, 0);

    let cipher = Aes256::new(GenericArray::from_slice(key));

    let mut iv1: [u8; 16] = iv[0..16].try_into().unwrap();
    let mut iv2: [u8; 16] = iv[16..32].try_into().unwrap();

    let mut next_iv1 = [0u8; 16];

    for block in buffer.chunks_mut(16) {
        // next iv1 = block (ciphertext)
        next_iv1.copy_from_slice(block);

        // block (ciphertext) XOR iv2 (previous plaintext)
        for i in 0..16 {
            block[i] ^= iv2[i]
        }

        cipher.decrypt_block(GenericArray::from_mut_slice(block));

        // block (plaintext) XOR iv1 (previous ciphertext)
        for i in 0..16 {
            block[i] ^= iv1[i]
        }

        // iv1 = next iv1 (ciphertext)
        mem::swap(&mut iv1, &mut next_iv1);

        // iv2 = block (plaintext)
        iv2.copy_from_slice(block);
    }
}
