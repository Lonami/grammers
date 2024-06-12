// Copyright 2020 - developers of the `grammers` project.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// https://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or https://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

#![deny(unsafe_code)]

//! This library is an implementation of the [Mobile Transport Protocol].
//!
//! It is capable of efficiently packing enqueued requests into message
//! containers to later be encrypted and transmitted, and processing the
//! server responses to maintain a correct state.
//!
//! [Mobile Transport Protocol]: https://core.telegram.org/mtproto
pub mod authentication;
mod manual_tl;
pub mod mtp;
pub mod transport;
mod utils;

/// The default compression threshold to be used.
pub const DEFAULT_COMPRESSION_THRESHOLD: Option<usize> = Some(512);

/// A Message Identifier.
///
/// When requests are enqueued, a new associated message identifier is
/// returned. As server responses get processed, some of them will be a
/// response to a previous request. You can now  `pop_response` to get
/// all the server responses, and if one matches your original identifier,
/// you will know the response corresponds to it.
#[derive(Copy, Clone, Debug, Hash, PartialEq, PartialOrd, Eq, Ord)]
pub struct MsgId(i64);
