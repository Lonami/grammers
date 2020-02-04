// Copyright 2020 - developers of the `grammers` project.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// https://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or https://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

//! This crate provides the [`Session`] trait, which allows client instances
//! to configure themselves with it.
//!
//! Sessions are used to persist the information required to connect to
//! Telegram to disk, so that the expensive process of initializing the
//! connection has only to be done once.
//!
//! [`Session`]: trait.session.html

mod memory_session;
mod session;
mod text_session;

pub use memory_session::MemorySession;
pub use session::Session;
pub use text_session::TextSession;
