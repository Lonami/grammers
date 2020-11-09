// Copyright 2020 - developers of the `grammers` project.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// https://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or https://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

//! This library is a high-level implementation to access [Telegram's API], which essentially
//! lets you automate everything you can do with official Telegram clients and more from Rust,
//! or even control bot accounts, making it a viable alternative to using the [Telegram Bot API].
//!
//! In order to create an application with the library for people to use, you will need to
//! [obtain a developer API ID] first. You can embed it as a constant in your binary and ship
//! that to users (anyone can login, including yourself and bots, with the developer API ID;
//! they do *not* need to provide their own API ID).
//!
//! Once that's ready, connect a new [`Client`] and start making API calls. You may also want to
//! import some [extension traits] to make the experience less bitter.
//!
//! [Telegram's API]: https://core.telegram.org/#telegram-api
//! [Telegram Bot API]: https://core.telegram.org/bots/api
//! [obtain a developer API ID]: https://my.telegram.org/auth
//! [`Client`]: struct.Client.html
//! [extension traits]: ext/index.html
mod client;
pub mod ext;
pub mod types;

pub use client::{Client, ClientHandle, Config, InitParams, UpdateIter};
pub use types::EntitySet;
