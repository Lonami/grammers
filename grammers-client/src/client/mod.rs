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
mod messages;
mod net;
mod retry_policy;
mod updates;

pub use auth::{LoginToken, PasswordToken, SignInError};
pub use bots::{InlineResult, InlineResultIter};
pub use chats::{ParticipantIter, ParticipantPermissions, ProfilePhotoIter};
pub(crate) use client::ClientInner;
pub use client::{Client, ClientConfiguration, UpdatesConfiguration};
pub use dialogs::DialogIter;
pub use files::DownloadIter;
pub use iter_buffer::IterBuffer;
pub use messages::{GlobalSearchIter, MessageIter, SearchIter};
pub use retry_policy::{AutoSleep, NoRetries, RetryContext, RetryPolicy};
pub use updates::UpdateStream;
