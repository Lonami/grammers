// Copyright 2020 - developers of the `grammers` project.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// https://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or https://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.
use num_bigint::BigUint;

use crate::{aes::ige_encrypt, sha256};

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

/// Increment data by 1 when interpreted as a big-endian big int.
fn increment(data: &mut [u8]) {
    let mut i = data.len() - 1;
    loop {
        let (n, overflow) = data[i].overflowing_add(1);
        data[i] = n;
        if overflow {
            if let Some(ni) = i.checked_sub(1) {
                i = ni
            } else {
                i = data.len() - 1;
            }
        } else {
            break;
        }
    }
}

/// Encrypt the given data, prefixing it with a hash before, using RSA.
pub fn encrypt_hashed(data: &[u8], key: &Key, random_bytes: &[u8; 224]) -> Vec<u8> {
    // https://core.telegram.org/mtproto/auth_key#41-rsa-paddata-server-public-key-mentioned-above-is-implemented-as-follows

    // data_with_padding := data + random_padding_bytes; -- where random_padding_bytes are chosen so that the resulting length of data_with_padding is precisely 192 bytes, and data is the TL-serialized data to be encrypted as before. One has to check that data is not longer than 144 bytes.
    assert!(data.len() <= 144);
    let data_with_padding = {
        let mut buffer = Vec::with_capacity(192);
        buffer.extend(data);
        buffer.extend(&random_bytes[..192 - data.len()]);
        buffer
    };

    // data_pad_reversed := BYTE_REVERSE(data_with_padding); -- is obtained from data_with_padding by reversing the byte order.
    let data_pad_reversed = data_with_padding.iter().copied().rev().collect::<Vec<u8>>();

    // a random 32-byte temp_key is generated.
    let mut temp_key: [u8; 32] = random_bytes[192..192 + 32].try_into().unwrap();

    let key_aes_encrypted = loop {
        // data_with_hash := data_pad_reversed + SHA256(temp_key + data_with_padding); -- after this assignment, data_with_hash is exactly 224 bytes long.
        let mut data_with_hash = {
            let mut buffer = Vec::with_capacity(224);
            buffer.extend(&data_pad_reversed);
            buffer.extend(sha256!(&temp_key, &data_with_padding));
            buffer
        };

        // aes_encrypted := AES256_IGE(data_with_hash, temp_key, 0); -- AES256-IGE encryption with zero IV.
        ige_encrypt(data_with_hash.as_mut(), &temp_key, &[0u8; 32]);
        let aes_encrypted = data_with_hash;

        // temp_key_xor := temp_key XOR SHA256(aes_encrypted); -- adjusted key, 32 bytes
        let temp_key_xor = {
            let mut xored = temp_key;
            xored
                .iter_mut()
                .zip(sha256!(&aes_encrypted))
                .for_each(|(a, b)| *a ^= b);
            xored
        };

        // key_aes_encrypted := temp_key_xor + aes_encrypted; -- exactly 256 bytes (2048 bits) long
        let key_aes_encrypted = {
            let mut buffer = Vec::with_capacity(256);
            buffer.extend(temp_key_xor);
            buffer.extend(aes_encrypted);
            buffer
        };

        // The value of key_aes_encrypted is compared with the RSA-modulus of server_pubkey as a big-endian 2048-bit (256-byte) unsigned integer.
        if BigUint::from_bytes_be(&key_aes_encrypted) >= key.n {
            // If key_aes_encrypted turns out to be greater than or equal to the RSA modulus, the previous steps starting from the generation of new random temp_key are repeated.
            increment(&mut temp_key);
            continue;
        }

        // Otherwise the final step is performed:
        break key_aes_encrypted;
    };

    // encrypted_data := RSA(key_aes_encrypted, server_pubkey); -- 256-byte big-endian integer is elevated to the requisite power from the RSA public key modulo the RSA modulus, and the result is stored as a big-endian integer consisting of exactly 256 bytes (with leading zero bytes if required).
    let payload = BigUint::from_bytes_be(&key_aes_encrypted);
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
    use crate::hex;

    #[test]
    fn test_rsa_encryption() {
        let key = Key::new("25342889448840415564971689590713473206898847759084779052582026594546022463853940585885215951168491965708222649399180603818074200620463776135424884632162512403163793083921641631564740959529419359595852941166848940585952337613333022396096584117954892216031229237302943701877588456738335398602461675225081791820393153757504952636234951323237820036543581047826906120927972487366805292115792231423684261262330394324750785450942589751755390156647751460719351439969059949569615302809050721500330239005077889855323917509948255722081644689442127297605422579707142646660768825302832201908302295573257427896031830742328565032949", "65537").unwrap();
        let result = encrypt_hashed(
            &hex::from_hex("955ff5a9081a8e635f5743de9b00000004453dc27100000004622f1fcb000000f7a81627bbf511fa4afef71e94a0937474586c1add9198dda81a5df8393871c8293623c5fb968894af1be7dfe9c7be813f9307789242fd0cb0c16a5cb39a8d3e"),
            &key,
            hex::from_hex("12270000635593b03fee033d0672f9afddf9124de9e77df6251806cba93482e4c9e6e06e7d44e4c4baae821aff91af44789689faaee9bdfc7b2df8c08709afe57396c4638ceaa0dc30114f82447e81d3b53edc423b32660c43a5b8ad057b64500000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000007dada0920c4973913229e0f881aec7b9db0c392d34f52fb0995ea493ecb4c09d").as_slice().try_into().unwrap(),
        );
        assert_eq!(
            result,
            hex::from_hex("b610642a828b4a61fe32931815cae318d311660580f1e0df768f3140f4d37dfcfcac0c2870318de4ff2d2e0e9669bcfdc0bad06cadb1b59d9726b427368a9c7b4fc0d5e7b2e99fc571968705c03acf5341fd7021bef653fa77b3776ae430e366fc46d232459ebe128b08d80e049ae579a48b56ca93b520709468587c81af96666046e9ea85091d729e921e8d8a36f57b27644052dae7387c7f4131701d59cda75251dac66c94276280ef950d3c44c21e5a2454f7da7a6818cf23ae9c490b72b2170d7cbc24f8a93db739d76f2d241c78b80123faaff3e664f074d6375d794dbf2800a0b5bb48d54eceafedfb355bfbebd287d9023264e3b53627888250787a9e")
        );
    }
}
