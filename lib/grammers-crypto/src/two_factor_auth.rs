// Copyright 2020 - developers of the `grammers` project.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// https://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or https://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.
use hmac::Hmac;
use num_bigint::BigUint;
use sha2::digest::Output;
use sha2::{Digest, Sha256, Sha512};

// H(data) := sha256(data)
macro_rules! h {
    ( $( $x:expr ),* ) => {
        {
            let mut hasher = Sha256::new();
            $(
                hasher.update($x);
            )*
            hasher.finalize()
        }
    };
}

pub fn calculate_2fa(
    salt1: Vec<u8>,
    salt2: Vec<u8>,
    g: i32,
    p: Vec<u8>,
    g_b: Vec<u8>,
    a: Vec<u8>,
    password: Vec<u8>,
) -> (Vec<u8>, Vec<u8>) {
    // Prepare our parameters
    let g_b = pad_to_256(&g_b);
    let a = pad_to_256(&a);

    let g_for_hash = vec![g as u8];
    let g_for_hash = pad_to_256(&g_for_hash);

    let big_g_b = BigUint::from_bytes_be(&g_b);

    let big_g = BigUint::from(g as u32);
    let big_a = BigUint::from_bytes_be(&a);
    let big_p = BigUint::from_bytes_be(&p);

    // k := H(p | g)
    let k = h!(&p, &g_for_hash);
    let big_k = BigUint::from_bytes_be(&k);

    // g_a := pow(g, a) mod p
    let g_a = big_g.modpow(&big_a, &big_p);
    let g_a = pad_to_256(&g_a.to_bytes_be());

    // u := H(g_a | g_b)
    let u = h!(&g_a, &g_b);
    let u = BigUint::from_bytes_be(&u);

    // x := PH2(password, salt1, salt2)
    let x = ph2(&password, &salt1, &salt2);
    let x = BigUint::from_bytes_be(&x);

    // v := pow(g, x) mod p
    let big_v = big_g.modpow(&x, &big_p);

    // k_v := (k * v) mod p
    let k_v = (big_k * big_v) % &big_p;

    // t := (g_b - k_v) mod p (positive modulo, if the result is negative increment by p)
    let sub = if big_g_b > k_v {
        big_g_b - k_v
    } else {
        k_v - big_g_b
    };
    let big_t = sub % &big_p;

    // s_a := pow(t, a + u * x) mod p
    let first = u * x;
    let second = big_a + first;
    let big_s_a = big_t.modpow(&(second), &big_p);

    // k_a := H(s_a)
    let k_a = h!(&pad_to_256(&big_s_a.to_bytes_be()));

    // M1 := H(H(p) xor H(g) | H(salt1) | H(salt2) | g_a | g_b | k_a)
    let h_p = h!(&p);
    let h_g = h!(&g_for_hash);

    let p_xor_g: Vec<u8> = xor(&h_p, &h_g);

    let m1 = h!(&p_xor_g, &h!(&salt1), &h!(&salt2), &g_a, &g_b, &k_a).to_vec();

    (m1, g_a)
}

// SH(data, salt) := H(salt | data | salt)
fn sh(data: impl AsRef<[u8]>, salt: impl AsRef<[u8]>) -> Output<Sha256> {
    return h!(&salt, &data, &salt);
}

// PH1(password, salt1, salt2) := SH(SH(password, salt1), salt2)
fn ph1(password: &Vec<u8>, salt1: &Vec<u8>, salt2: &Vec<u8>) -> Output<Sha256> {
    sh(&sh(password, salt1), salt2)
}

// PH2(password, salt1, salt2)
//                      := SH(pbkdf2(sha512, PH1(password, salt1, salt2), salt1, 100000), salt2)
fn ph2(password: &Vec<u8>, salt1: &Vec<u8>, salt2: &Vec<u8>) -> Output<Sha256> {
    let hash1 = ph1(password, salt1, salt2);

    // 512-bit derived key
    let mut dk = [0u8; 64];
    pbkdf2::pbkdf2::<Hmac<Sha512>>(&hash1, salt1, 100000, &mut dk);

    sh(&dk, salt2)
}

fn xor(left: &Output<Sha256>, right: &Output<Sha256>) -> Vec<u8> {
    return left
        .iter()
        .zip(right.iter())
        .map(|(&x1, &x2)| x1 ^ x2)
        .collect();
}

fn pad_to_256(data: &Vec<u8>) -> Vec<u8> {
    let mut new_vec = data.clone();
    for _ in 0..(256 - new_vec.len()) {
        new_vec.insert(0, 0);
    }
    new_vec
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn check_calculations() {
        let salt1 = vec![1];
        let salt2 = vec![2];
        let g = 3;
        let p = pad_to_256(&vec![4]);
        let g_b = vec![5];
        let a = vec![6];
        let password = vec![7];

        let (m1, g_a) = calculate_2fa(salt1, salt2, g, p, g_b, a, password);

        let expected_m1 = vec![
            113, 194, 128, 151, 4, 153, 170, 134, 32, 95, 223, 56, 223, 136, 52, 244, 208, 194,
            114, 97, 231, 249, 72, 123, 225, 229, 225, 113, 128, 184, 98, 51,
        ];
        let expected_g_a = vec![
            0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1,
        ];

        assert_eq!(expected_m1, m1);
        assert_eq!(expected_g_a, g_a);
    }
}
