// Copyright 2020 - developers of the `grammers` project.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// https://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or https://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.
pub mod auth;
pub mod bots;
pub mod chats;
pub mod client;
pub mod dialogs;
pub mod files;
pub mod messages;
pub mod net;
pub mod updates;

pub use auth::SignInError;
pub(crate) use client::ClientInner;
pub use client::{Client, Config, InitParams};
