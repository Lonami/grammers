// Copyright 2020 - developers of the `grammers` project.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// https://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or https://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

//! # Brief on Telegram's API
//!
//! This library is a high-level implementation to access [Telegram's API], which essentially
//! lets you automate everything you can do with official Telegram clients and more from Rust,
//! or even control bot accounts, making it a viable alternative to using the [Telegram Bot API].
//!
//! In order to create an application with the library for people to use, you will need to
//! [obtain a developer API ID] first. You can embed it as a constant in your binary and ship
//! that to users (anyone can login, including yourself and bots, with the developer API ID;
//! they do *not* need to provide their own API ID).
//!
//! Once that's ready, create a new [`Client`] instance with its [`Client::new`]
//! method and start making API calls.
//!
//! # Method cost
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
//! There is nothing wrong with causing the API to return flood wait errors, but it is something
//! to be avoided. Because they are expected, the default [`ClientConfiguration`] will sleep on
//! small flood errors to prevent interruption of simple scripts.
//!
//! A flood wait error is different from a peer flood error. A peer flood error means the flood
//! limitation is applied account-wide, and its duration is undefined. This often means that the
//! account spammed, or a young account tried to contact too many peers.
//!
//! # Re-exports
//!
//! For convenience and to ease using compatible traits in lockstep, this
//! library re-exports several modules and types from its direct dependencies.
//!
//! ## grammers-mtsender as sender
//!
//! `grammers_client` is purely a friendlier abstraction on top of the [`SenderPool`],
//! which is responsible for abstracting the connection wholesale. The [`InvocationError`]
//! is also re-exported at the top level, as all methods that interact with the API
//! eventually need to invoke the request and may thus fail. You may also use the
//! [`Result`] alias, which has the `InvocationError` set as its error variant.
//!
//! If you need access to the rest of the crate (e.g. to configure the `SenderPool`),
//! it is re-exported under [`sender`].
//!
//! ## grammers-session as session
//!
//! After login, the generated Authorization Key should be persisted and reused,
//! in order to avoid logging in again, as that is a very expensive method.
//!
//! Session storages are responsible for doing so, and you may even implement your
//! own by using the [`session::Session`] trait.
//!
//! ## grammers-tl-types as tl
//!
//! This crate is re-exported wholesale under the [`tl`] namespace (short for
//! [Type Language](https://core.telegram.org/mtproto/TL)). All Telegram types and functions
//! are automatically generated from their public schema, which is versioned by "layers".
//! For now and the foreseeable future, *grammers* will only offer one layer at a time.
//!
//! When the friendly API of `grammers_client` falls short, you can reach out for its
//! raw [`tl::types`] to access all the information Telegram returned on its responses.
//! Additionally, if the [`Client`] is missing a method to do what you need, you may
//! directly [`Client::invoke`] any of the [`tl::functions`] offered by Telegram's API.
//!
//! Using raw types is **discouraged**. Not only are they far clunkier to use, but they
//! are also **not part of the semantic versioning**. While patch versions won't change
//! the layer, minor versions will, which would traditionally be considered a breaking change.
//!
//! [Telegram's API]: https://core.telegram.org/#telegram-api
//! [Telegram Bot API]: https://core.telegram.org/bots/api
//! [obtain a developer API ID]: https://my.telegram.org/auth
//! [`ClientConfiguration`]: crate::client::ClientConfiguration

#![deny(unsafe_code)]

pub mod client;
pub mod media;
pub mod message;
#[cfg(any(feature = "markdown", feature = "html"))]
pub mod parsers;
pub mod peer;
pub mod update;
pub(crate) mod utils;

pub use client::{Client, SignInError};
pub use grammers_mtsender::{self as sender, InvocationError, SenderPool};
pub use grammers_session as session;
pub use grammers_tl_types as tl;

/// Alias for [`std::result::Result`] with the error set to [`InvocationError`].
pub type Result<T> = std::result::Result<T, InvocationError>;
