// Copyright 2020 - developers of the `grammers` project.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// https://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or https://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

/// Generate a random ID suitable for sending messages or media.
pub(crate) fn generate_random_id() -> i64 {
    let mut buffer = [0; 8];
    getrandom::getrandom(&mut buffer).expect("failed to generate random message id");
    i64::from_le_bytes(buffer)
}

pub(crate) fn generate_random_ids(n: usize) -> Vec<i64> {
    let start = generate_random_id();
    (0..n as i64).map(|i| start.wrapping_add(i)).collect()
}
