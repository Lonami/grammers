// Copyright 2020 - developers of the `grammers` project.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// https://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or https://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

//! Update types about events received from Telegram.
//!
//! Properties containing raw types are public and will either be called "raw" or prefixed with "raw_".\
//! Keep in mind that **these fields are not part of the semantic versioning guarantees**.

mod callback_query;
mod inline_query;
mod inline_send;
mod message;
mod message_deletion;
mod raw;
mod update;

pub use callback_query::CallbackQuery;
pub use inline_query::{Article, InlineQuery};
pub use inline_send::InlineSend;
pub use message::Message;
pub use message_deletion::MessageDeletion;
pub use raw::Raw;
pub use update::Update;
