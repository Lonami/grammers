// Copyright 2020 - developers of the `grammers` project.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// https://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or https://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

use std::fmt::Write;

/// Represent a sequence of bytes as an hexadecimal string.
pub fn to_hex(bytes: &[u8]) -> String {
    let mut result = String::with_capacity(bytes.len() * 2);
    bytes.iter().for_each(|b| {
        write!(result, "{:02x}", b).unwrap();
    });
    result
}

/// Convert an hexadecimal string into a sequence of bytes.
pub fn opt_from_hex(hex: &str) -> Option<Vec<u8>> {
    fn hex_to_decimal(hex_digit: u8) -> Option<u8> {
        Some(match hex_digit {
            b'0'..=b'9' => hex_digit - b'0',
            b'a'..=b'f' => hex_digit - b'a' + 0xa,
            b'A'..=b'F' => hex_digit - b'A' + 0xa,
            _ => return None,
        })
    }

    if hex.len() % 2 != 0 {
        return None;
    }

    hex.as_bytes()
        .chunks_exact(2)
        .map(
            |slice| match (hex_to_decimal(slice[0]), hex_to_decimal(slice[1])) {
                (Some(h), Some(l)) => Some(h * 0x10 + l),
                _ => None,
            },
        )
        .collect()
}

/// Like `opt_from_hex`, but panics on invalid data.
pub fn from_hex(hex: &str) -> Vec<u8> {
    opt_from_hex(hex).unwrap()
}
