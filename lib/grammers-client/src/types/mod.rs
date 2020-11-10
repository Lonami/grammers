// Copyright 2020 - developers of the `grammers` project.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// https://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or https://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

//! Custom types extending those provided by Telegram.
mod dialog;
mod entity;
mod entity_set;
mod message;
mod sent_code;

pub use dialog::{Dialog, DialogIter};
pub use entity::Entity;
pub use entity_set::EntitySet;
pub use message::Message;
pub use sent_code::SentCode;
