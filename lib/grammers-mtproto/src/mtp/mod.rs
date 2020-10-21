// Copyright 2020 - developers of the `grammers` project.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// https://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or https://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

//! Implementation of the [Mobile Transport Protocol]. This layer is
//! responsible for converting zero or more input requests into outgoing
//! messages, and to process the response.
//!
//! A distinction between plain and encrypted is made for simplicity (the
//! plain hardly requires to process any state) and to help prevent invalid
//! states (encrypted communication cannot be made without an authorization
//! key).
//!
//! [Mobile Transport Protocol]: https://core.telegram.org/mtproto/description

pub trait Mtp {}
