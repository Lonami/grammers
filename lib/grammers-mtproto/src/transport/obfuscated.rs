// Copyright 2020 - developers of the `grammers` project.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// https://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or https://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.
use grammers_crypto::{obfuscated::ObfuscatedCipher, DequeBuffer};
use log::debug;

use super::{Error, Tagged, Transport, UnpackedOffset};

/// An obfuscation protocol made by telegram to avoid ISP blocks.
/// This is needed to connect to the Telegram servers using websockets or
/// when conecting to MTProto proxies (not yet supported).
///
/// It is simply a wrapper around another transport, which encrypts the data
/// using AES-256-CTR with a randomly generated key that is then sent at the
/// beginning of the connection.
///
/// Obfuscated transport can only be used with "tagged" transports, which
/// provide a way to get the obfuscated tag that is used in the encryption.
/// See the linked documentation for more information.
///
/// [Transport Obfuscation](https://core.telegram.org/mtproto/mtproto-transports#transport-obfuscation)
pub struct Obfuscated<T: Transport + Tagged> {
    inner: T,
    head: Option<[u8; 64]>,
    decrypt_tail: usize,
    cipher: ObfuscatedCipher,
}

const FORBIDDEN_FIRST_INTS: [[u8; 4]; 7] = [
    [b'H', b'E', b'A', b'D'], // HTTP HEAD
    [b'P', b'O', b'S', b'T'], // HTTP POST
    [b'G', b'E', b'T', b' '], // HTTP GET
    [b'O', b'P', b'T', b'I'], // HTTP OPTIONS
    [0x16, 0x03, 0x01, 0x02], // TLS handshake
    [0xdd, 0xdd, 0xdd, 0xdd], // Padded Intermediate
    [0xee, 0xee, 0xee, 0xee], // Intermediate
];

impl<T: Transport + Tagged> Obfuscated<T> {
    fn generate_keys(inner: &mut T) -> ([u8; 64], ObfuscatedCipher) {
        let mut init = [0; 64];

        while init[4..8] == [0; 4] // Full
            || init[0] == 0xef // Abridged
            || FORBIDDEN_FIRST_INTS.iter().any(|start| start == &init[..4])
        {
            getrandom::getrandom(&mut init).unwrap();
        }

        init[56..60].copy_from_slice(&inner.init_tag());

        let mut cipher = ObfuscatedCipher::new(&init);

        let mut encrypted_init = init.to_vec();
        cipher.encrypt(&mut encrypted_init);
        init[56..64].copy_from_slice(&encrypted_init[56..64]);

        (init, cipher)
    }

    pub fn new(mut inner: T) -> Self {
        let (init, cipher) = Self::generate_keys(&mut inner);

        Self {
            inner,
            head: Some(init),
            decrypt_tail: 0,
            cipher,
        }
    }
}

impl<T: Transport + Tagged> Transport for Obfuscated<T> {
    fn pack(&mut self, buffer: &mut DequeBuffer<u8>) {
        self.inner.pack(buffer);
        self.cipher.encrypt(buffer.as_mut());
        if let Some(head) = self.head.take() {
            buffer.extend_front(&head);
        }
    }

    fn unpack(&mut self, buffer: &mut [u8]) -> Result<UnpackedOffset, Error> {
        if buffer.len() < self.decrypt_tail {
            panic!("buffer is smaller than what was decrypted");
        }

        self.cipher.decrypt(&mut buffer[self.decrypt_tail..]);
        self.decrypt_tail = buffer.len();

        match self.inner.unpack(buffer) {
            Ok(offset) => {
                self.decrypt_tail -= offset.next_offset;
                Ok(offset)
            }
            Err(e) => Err(e),
        }
    }

    fn reset(&mut self) {
        self.inner.reset();
        debug!("regenerating keys for obfuscated transport");

        let (init, cipher) = Self::generate_keys(&mut self.inner);
        self.head = Some(init);
        self.cipher = cipher;
    }
}
