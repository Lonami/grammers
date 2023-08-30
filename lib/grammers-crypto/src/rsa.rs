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

/// Encrypt the given data, prefixing it with a hash before, using RSA.
pub fn encrypt_hashed(data: &[u8], key: &Key, random_bytes: &[u8; 256]) -> Vec<u8> {
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

    let mut attempt = 0;
    let key_aes_encrypted = loop {
        if 192 + 32 * attempt + 32 > random_bytes.len() {
            panic!("ran out of entropy");
        }

        // a random 32-byte temp_key is generated.
        let temp_key = &random_bytes[192 + 32 * attempt..192 + 32 * attempt + 32]
            .try_into()
            .unwrap();

        // data_with_hash := data_pad_reversed + SHA256(temp_key + data_with_padding); -- after this assignment, data_with_hash is exactly 224 bytes long.
        let data_with_hash = {
            let mut buffer = Vec::with_capacity(224);
            buffer.extend(&data_pad_reversed);
            buffer.extend(sha256!(&temp_key, &data_with_padding));
            buffer
        };

        // aes_encrypted := AES256_IGE(data_with_hash, temp_key, 0); -- AES256-IGE encryption with zero IV.
        let aes_encrypted = ige_encrypt(&data_with_hash, &temp_key, &[0u8; 32]);

        // temp_key_xor := temp_key XOR SHA256(aes_encrypted); -- adjusted key, 32 bytes
        let temp_key_xor = {
            let mut xored = temp_key.clone();
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

        // The value of key_aes_encrypted is compared with the RSA-modulus of server_pubkey as a big-endian 2048-bit (256-byte) unsigned integer. If key_aes_encrypted turns out to be greater than or equal to the RSA modulus, the previous steps starting from the generation of new random temp_key are repeated. Otherwise the final step is performed:
        if BigUint::from_bytes_be(&key_aes_encrypted) < key.n {
            break key_aes_encrypted;
        }

        attempt += 1;
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
            hex::from_hex("12270000635593b03fee033d0672f9afddf9124de9e77df6251806cba93482e4c9e6e06e7d44e4c4baae821aff91af44789689faaee9bdfc7b2df8c08709afe57396c4638ceaa0dc30114f82447e81d3b53edc423b32660c43a5b8ad057b64500000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000007dada0920c4973913229e0f881aec7b9db0c392d34f52fb0995ea493ecb4c09daaf68fe9554ec7a59c03e4035952b220b47a8d06aad71134110d8c44948901f8").as_slice().try_into().unwrap(),
        );
        assert_eq!(
            result,
            hex::from_hex("c6d211349fc10cda6983276250b09f4be9b39f533b5d314b732b51a6dd72234dab4224209992c894e0e4c9f30249f1dbbd1630a27b98f2f92a53c00baabbd46f380bd35f417e5ec2edb43f7644b5c81af011d736eb369265e848b553ae5e6350dd5695efc72bde0e35f3c3fc827b91eb97cf1efdbff12269b9c33f81645adebc89ed167edc19d285237a754bf629aa358ed08498863b2aec8b7139001627bbe8bdef239474a5a43e664d278f39e72d694a206d7b838fd40868a71c4bfbffa38b7679faa502b7795cbe5ae1bd05ca7eb01ff5b05107265fd39bd5b4e19d392b735a3b0b5b21473062981bff86ff9084a7b594775e3127c05fd454e19f794a4ab4")
        );
    }
}
