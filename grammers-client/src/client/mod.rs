// Copyright 2020 - developers of the `grammers` project.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// https://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or https://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

//! Contains client-specific configuration and types.

mod auth;
mod bots;
mod chats;
#[allow(clippy::module_inception)]
mod client;
mod dialogs;
mod files;
mod iter_buffer;
mod login_token;
mod messages;
mod net;
mod password_token;
mod retry_policy;
mod updates;

pub use auth::SignInError;
pub(crate) use client::ClientInner;
pub use client::{Client, ClientConfiguration, UpdatesConfiguration};
pub use iter_buffer::IterBuffer;
pub use login_token::LoginToken;
pub use password_token::PasswordToken;
pub use retry_policy::{AutoSleep, NoRetries, RetryContext, RetryPolicy};
