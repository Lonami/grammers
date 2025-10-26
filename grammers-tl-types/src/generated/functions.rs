// Copyright 2020 - developers of the `grammers` project.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// https://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or https://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

#![allow(
    clippy::cognitive_complexity,
    clippy::identity_op,
    clippy::unreadable_literal
)]

//! All of the functions, each represented by a `struct`.
//!
//! All of them implement [`crate::Identifiable`] and [`crate::Serializable`]
//! (and, when the feature is enabled, [`crate::Deserializable`]).
//!
//! To find out the type that Telegram will return upon
//! invoking one of these requests, check out the associated
//! type in the corresponding [`crate::RemoteCall`] trait impl.

include!(concat!(env!("OUT_DIR"), "/generated_functions.rs"));
