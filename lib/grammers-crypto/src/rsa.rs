// Copyright 2020 - developers of the `grammers` project.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// https://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or https://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.
use num_bigint::BigUint;
use sha1::{Digest, Sha1};

/// RSA key.
pub struct Key {
    n: BigUint,
    e: BigUint,
}

impl Key {
    pub fn new(n: &str, e: &str) -> Option<Self> {
        Some(Self {
            n: BigUint::parse_bytes(n.as_bytes(), 10)?,
            e: BigUint::parse_bytes(e.as_bytes(), 10)?,
        })
    }
}

/// Encrypt the given data, prefixing it with a hash before, using RSA.
pub fn encrypt_hashed(data: &[u8], key: &Key, random_bytes: &[u8; 256]) -> Vec<u8> {
    // Sha1::digest's len is always 20, we're left with 255 - 20 - x padding.
    let sha = {
        let mut hasher = Sha1::new();
        hasher.update(&data);
        hasher.finalize()
    };

    let to_encrypt = {
        // sha1
        let mut buffer = Vec::with_capacity(255);
        buffer.extend(sha);

        // + data
        buffer.extend(data);

        // + padding
        let padding_len = 255 - 20 - data.len();
        let mut padding = vec![0; padding_len];
        padding.copy_from_slice(&random_bytes[..padding_len]);
        buffer.extend(&padding);

        buffer
    };

    let payload = BigUint::from_bytes_be(&to_encrypt);
    let encrypted = payload.modpow(&key.e, &key.n);
    let mut block = encrypted.to_bytes_be();
    while block.len() < 256 {
        block.insert(0, 0);
    }

    block
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rsa_encryption() {
        let key = Key::new("22081946531037833540524260580660774032207476521197121128740358761486364763467087828766873972338019078976854986531076484772771735399701424566177039926855356719497736439289455286277202113900509554266057302466528985253648318314129246825219640197356165626774276930672688973278712614800066037531599375044750753580126415613086372604312320014358994394131667022861767539879232149461579922316489532682165746762569651763794500923643656753278887871955676253526661694459370047843286685859688756429293184148202379356802488805862746046071921830921840273062124571073336369210703400985851431491295910187179045081526826572515473914151", "65537").unwrap();
        let result = encrypt_hashed(b"Hello!", &key, &[0; 256]);
        assert_eq!(
            result,
            vec![
                117, 112, 45, 76, 136, 210, 155, 106, 185, 52, 53, 81, 36, 221, 40, 217, 182, 42,
                71, 85, 136, 65, 200, 3, 20, 80, 247, 73, 155, 28, 156, 107, 211, 157, 39, 193, 88,
                28, 81, 52, 78, 81, 193, 121, 35, 112, 100, 167, 35, 174, 147, 157, 90, 195, 80,
                20, 253, 139, 79, 226, 79, 117, 227, 17, 92, 50, 161, 99, 105, 238, 43, 55, 58, 97,
                236, 148, 70, 185, 43, 46, 61, 240, 118, 24, 219, 10, 138, 253, 169, 153, 182, 112,
                43, 50, 181, 129, 155, 214, 234, 73, 112, 251, 52, 124, 168, 74, 96, 208, 195, 138,
                183, 12, 102, 229, 237, 1, 64, 68, 136, 137, 163, 184, 130, 238, 165, 51, 186, 208,
                94, 250, 32, 69, 237, 167, 23, 18, 60, 65, 74, 191, 222, 212, 62, 30, 180, 131,
                160, 73, 120, 110, 245, 3, 27, 18, 213, 26, 63, 247, 236, 183, 216, 4, 212, 65, 53,
                148, 95, 152, 247, 90, 74, 108, 241, 161, 223, 55, 85, 158, 48, 187, 233, 42, 75,
                121, 102, 195, 79, 7, 56, 230, 209, 48, 89, 133, 119, 109, 38, 223, 171, 124, 15,
                223, 215, 236, 32, 44, 199, 140, 84, 207, 130, 172, 35, 134, 199, 157, 14, 25, 117,
                128, 164, 250, 148, 48, 10, 35, 130, 249, 225, 22, 254, 130, 223, 155, 216, 114,
                229, 185, 218, 123, 66, 98, 35, 191, 26, 216, 88, 137, 48, 181, 30, 22, 93, 108,
                221, 2
            ]
        );
    }
}
