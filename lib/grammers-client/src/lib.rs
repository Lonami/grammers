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
//! Once that's ready, connect a new [`Client`] and start making API calls.
//!
//! When a method is said to be "expensive", this often means that calling it too much in a
//! certain period of time will result in the API returning "flood wait" errors, meaning that
//! the method cannot be called again for a certain amount of seconds (trying to do so will
//! continue to return the flood wait error for that duration).
//!
//! On the other hand, a "cheap" method can be called a lot and is unlikely to result in a flood
//! wait error. However, all methods can potentially cause a flood wait, so some care is still
//! required.
//!
//! There is nothing wrong with causing the API to return flood wait errors, but it might be a
//! good idea to try and not hit them.
//!
//! A flood wait error is different from a peer flood error. A peer flood error means the flood
//! limitation is applied account-wide, and its duration is undefined. This often means that the
//! account spammed, or a young account tried to contact too many peers.
//!
//! [Telegram's API]: https://core.telegram.org/#telegram-api
//! [Telegram Bot API]: https://core.telegram.org/bots/api
//! [obtain a developer API ID]: https://my.telegram.org/auth
pub mod client;
mod parsers;
pub mod types;
pub(crate) mod utils;

pub use client::{Client, Config, InitParams, SignInError};
pub use types::{button, reply_markup, ChatMap, InputMessage, Update};
