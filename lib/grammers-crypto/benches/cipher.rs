// Copyright 2020 - developers of the `grammers` project.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// https://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or https://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.
use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use grammers_crypto::aes::{ige_decrypt, ige_encrypt};

fn bench_encrypt_ige(c: &mut Criterion) {
    let mut group = c.benchmark_group("IGE encryption (≤1KB)");

    for size in [16usize, 256, 512, 1024].iter().copied() {
        group.throughput(Throughput::Bytes(size as u64));

        let data = black_box(vec![1; size]);
        let key = black_box([2; 32]);
        let iv = black_box([3; 32]);

        group.bench_with_input(
            BenchmarkId::new("encrypt", size),
            &(data, key, iv),
            |b, (data, key, iv)| b.iter(|| ige_encrypt(data, key, iv)),
        );
    }

    group.finish();

    let mut group = c.benchmark_group("IGE encryption (>1KB)");
    group.sample_size(10);

    for size in [16 * 1024, 128 * 1024, 512 * 1024].iter().copied() {
        group.throughput(Throughput::Bytes(size as u64));

        let data = black_box(vec![1; size]);
        let key = black_box([2; 32]);
        let iv = black_box([3; 32]);

        group.bench_with_input(
            BenchmarkId::new("encrypt", size),
            &(data, key, iv),
            |b, (data, key, iv)| b.iter(|| ige_encrypt(data, key, iv)),
        );
    }

    group.finish();
}

fn bench_decrypt_ige(c: &mut Criterion) {
    let mut group = c.benchmark_group("IGE decryption (≤1KB)");

    for size in [16usize, 256, 512, 1024].iter().copied() {
        group.throughput(Throughput::Bytes(size as u64));

        let data = black_box(vec![1; size]);
        let key = black_box([2; 32]);
        let iv = black_box([3; 32]);

        group.bench_with_input(
            BenchmarkId::new("decrypt", size),
            &(data, key, iv),
            |b, (data, key, iv)| b.iter(|| ige_decrypt(data, key, iv)),
        );
    }

    group.finish();

    let mut group = c.benchmark_group("IGE decryption (>1KB)");
    group.sample_size(10);

    for size in [16 * 1024, 128 * 1024, 512 * 1024].iter().copied() {
        group.throughput(Throughput::Bytes(size as u64));

        let data = black_box(vec![1; size]);
        let key = black_box([2; 32]);
        let iv = black_box([3; 32]);

        group.bench_with_input(
            BenchmarkId::new("decrypt", size),
            &(data, key, iv),
            |b, (data, key, iv)| b.iter(|| ige_decrypt(data, key, iv)),
        );
    }

    group.finish();
}

criterion_group!(benches, bench_encrypt_ige, bench_decrypt_ige);
criterion_main!(benches);
