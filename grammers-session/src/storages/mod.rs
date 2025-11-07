// Copyright 2020 - developers of the `grammers` project.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// https://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or https://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

//! Built-in [`crate::Session`] storages.
//!
//! Some may require certain features to be enabled. If none fit
//! your needs, you can also implement [`crate::Session`] yourself.

mod memory;
#[cfg(feature = "sqlite-storage")]
mod sqlite;

pub use memory::MemorySession;
#[cfg(feature = "sqlite-storage")]
pub use sqlite::SqliteSession;
