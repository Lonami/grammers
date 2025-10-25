// Copyright 2020 - developers of the `grammers` project.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// https://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or https://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

#[macro_export]
macro_rules! sha1 (
    ( $( $x:expr ),* ) => ({
        use sha1::{Digest, Sha1};
        let mut hasher = Sha1::new();
        $(
            hasher.update($x);
        )+
        let sha: [u8; 20] = hasher.finalize().into();
        sha
    })
);

#[macro_export]
macro_rules! sha256 (
    ( $( $x:expr ),* ) => ({
        use sha2::{Digest, Sha256};
        let mut hasher = Sha256::new();
        $(
            hasher.update($x);
        )+
        let sha: [u8; 32] = hasher.finalize().into();
        sha
    })
);
