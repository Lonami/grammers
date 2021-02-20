// Copyright 2020 - developers of the `grammers` project.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// https://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or https://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.
use aes_soft::cipher::generic_array::GenericArray;
use aes_soft::cipher::{BlockCipher, NewBlockCipher};
#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
use once_cell::sync::Lazy;

#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
static USE_AES_NI: Lazy<bool> =
    Lazy::new(|| is_x86_feature_detected!("aes") && is_x86_feature_detected!("sse2"));

macro_rules! do_encrypt {
    ( $root:ident($plaintext:expr, $key:expr, $iv:expr) ) => {{
        assert!($plaintext.len() % 16 == 0);
        let mut ciphertext = Vec::with_capacity($plaintext.len());

        let key = GenericArray::from_slice($key);
        let cipher = $root::Aes256::new(&key);

        let mut iv = $iv.clone();
        let (iv1, iv2) = iv.split_at_mut(16);

        for plaintext_block in $plaintext.chunks(16) {
            // block = block XOR iv1
            let mut xored_block = [0; 16];
            xored_block
                .iter_mut()
                .zip(plaintext_block.iter().zip(iv1.iter()))
                .for_each(|(x, (a, b))| *x = a ^ b);

            // block = encrypt(block);
            let mut ciphertext_block = GenericArray::clone_from_slice(&xored_block);
            cipher.encrypt_block(&mut ciphertext_block);

            // block = block XOR iv2
            ciphertext_block
                .iter_mut()
                .zip(iv2.iter())
                .for_each(|(x, a)| *x ^= a);

            // save ciphertext and adjust iv
            ciphertext.extend(ciphertext_block.iter());
            iv1.clone_from_slice(&ciphertext_block);
            iv2.clone_from_slice(plaintext_block);
        }

        ciphertext
    }};
}

macro_rules! do_decrypt {
    ( $root:ident($ciphertext:expr, $key:expr, $iv:expr) ) => {{
        assert!($ciphertext.len() % 16 == 0);
        let mut plaintext = Vec::with_capacity($ciphertext.len());

        let key = GenericArray::from_slice($key);
        let cipher = $root::Aes256::new(&key);

        let mut iv = $iv.clone();
        let (iv1, iv2) = iv.split_at_mut(16);

        for ciphertext_block in $ciphertext.chunks(16) {
            // block = block XOR iv2
            let mut xored_block = [0; 16];
            xored_block
                .iter_mut()
                .zip(ciphertext_block.iter().zip(iv2.iter()))
                .for_each(|(x, (a, b))| *x = a ^ b);

            // block = decrypt(block);
            let mut plaintext_block = GenericArray::clone_from_slice(&xored_block);
            cipher.decrypt_block(&mut plaintext_block);

            // block = block XOR iv1
            plaintext_block
                .iter_mut()
                .zip(iv1.iter())
                .for_each(|(x, a)| *x ^= a);

            // save plaintext and adjust iv
            plaintext.extend(plaintext_block.iter());
            iv1.clone_from_slice(ciphertext_block);
            iv2.clone_from_slice(&plaintext_block);
        }

        plaintext
    }};
}

/// Encrypt the input plaintext using the AES-IGE mode.
pub fn ige_encrypt(plaintext: &[u8], key: &[u8; 32], iv: &[u8; 32]) -> Vec<u8> {
    #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
    {
        if *USE_AES_NI {
            do_encrypt!(aesni(plaintext, key, iv))
        } else {
            do_encrypt!(aes_soft(plaintext, key, iv))
        }
    }
    #[cfg(not(any(target_arch = "x86", target_arch = "x86_64")))]
    do_encrypt!(aes_soft(plaintext, key, iv))
}

/// Decrypt the input ciphertext using the AES-IGE mode.
pub fn ige_decrypt(ciphertext: &[u8], key: &[u8; 32], iv: &[u8; 32]) -> Vec<u8> {
    #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
    {
        if *USE_AES_NI {
            do_decrypt!(aesni(ciphertext, key, iv))
        } else {
            do_decrypt!(aes_soft(ciphertext, key, iv))
        }
    }
    #[cfg(not(any(target_arch = "x86", target_arch = "x86_64")))]
    do_decrypt!(aes_soft(ciphertext, key, iv))
}
