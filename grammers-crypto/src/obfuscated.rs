// Copyright 2020 - developers of the `grammers` project.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// https://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or https://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

#[allow(deprecated)] // see https://github.com/RustCrypto/block-ciphers/issues/509
use aes::cipher::{KeyIvInit, StreamCipher, generic_array::GenericArray};

/// This implements the AES-256-CTR cipher used by Telegram to encrypt data
/// when using the obfuscated transport.
///
/// It is not intended to be used directly.
pub struct ObfuscatedCipher {
    rx: ctr::Ctr128BE<aes::Aes256>,
    tx: ctr::Ctr128BE<aes::Aes256>,
}

impl ObfuscatedCipher {
    pub fn new(init: &[u8; 64]) -> Self {
        let init_rev = init.iter().copied().rev().collect::<Vec<_>>();
        #[allow(deprecated)] // see https://github.com/RustCrypto/block-ciphers/issues/509
        Self {
            rx: ctr::Ctr128BE::<aes::Aes256>::new(
                GenericArray::from_slice(&init_rev[8..40]),
                GenericArray::from_slice(&init_rev[40..56]),
            ),
            tx: ctr::Ctr128BE::<aes::Aes256>::new(
                GenericArray::from_slice(&init[8..40]),
                GenericArray::from_slice(&init[40..56]),
            ),
        }
    }

    pub fn encrypt(&mut self, buffer: &mut [u8]) {
        self.tx.apply_keystream(buffer);
    }

    pub fn decrypt(&mut self, buffer: &mut [u8]) {
        self.rx.apply_keystream(buffer);
    }
}
