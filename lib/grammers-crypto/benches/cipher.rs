// Copyright 2020 - developers of the `grammers` project.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// https://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or https://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.
use bencher::{benchmark_group, benchmark_main, black_box, Bencher};
use grammers_crypto::aes::{ige_decrypt, ige_encrypt};

macro_rules! define_benches {
    ($(fn $func:ident($method:ident, $n:expr);)+) => {
        $(
            fn $func(bench: &mut Bencher) {
                let data = black_box(vec![1; $n]);
                let key = black_box([2; 32]);
                let iv = black_box([3; 32]);

                bench.iter(|| {
                    black_box($method(&data, &key, &iv))
                });
                bench.bytes = data.len() as u64;

            }
        )+
    };
}

define_benches!(
    fn encrypt_b0016(ige_encrypt, 16);
    fn encrypt_b0256(ige_encrypt, 256);
    fn encrypt_b0512(ige_encrypt, 512);
    fn encrypt_b1024(ige_encrypt, 1024);

    fn encrypt_kb0016(ige_encrypt, 16 * 1024);
    fn encrypt_kb0128(ige_encrypt, 128 * 1024);
    fn encrypt_kb0512(ige_encrypt, 512 * 1024);

    fn decrypt_b0016(ige_decrypt, 16);
    fn decrypt_b0256(ige_decrypt, 256);
    fn decrypt_b0512(ige_decrypt, 512);
    fn decrypt_b1024(ige_decrypt, 1024);

    fn decrypt_kb0016(ige_decrypt, 16 * 1024);
    fn decrypt_kb0128(ige_decrypt, 128 * 1024);
    fn decrypt_kb0512(ige_decrypt, 512 * 1024);
);

benchmark_group!(
    encrypt_small,
    encrypt_b0016,
    encrypt_b0256,
    encrypt_b0512,
    encrypt_b1024
);
benchmark_group!(encrypt_big, encrypt_kb0016, encrypt_kb0128, encrypt_kb0512);
benchmark_group!(
    decrypt_small,
    decrypt_b0016,
    decrypt_b0256,
    decrypt_b0512,
    decrypt_b1024
);
benchmark_group!(decrypt_big, decrypt_kb0016, decrypt_kb0128, decrypt_kb0512);
benchmark_main!(encrypt_small, encrypt_big, decrypt_small, decrypt_big);
