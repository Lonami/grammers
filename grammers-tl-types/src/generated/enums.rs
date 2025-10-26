// Copyright 2020 - developers of the `grammers` project.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// https://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or https://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

#![allow(clippy::large_enum_variant)]

//! All of the boxed types, each represented by a `enum`.
//!
//! All of them implement [`crate::Serializable`] and [`crate::Deserializable`].

include!(concat!(env!("OUT_DIR"), "/generated_enums.rs"));
