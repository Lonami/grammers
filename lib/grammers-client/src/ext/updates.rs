// Copyright 2020 - developers of the `grammers` project.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// https://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or https://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.
use crate::types::Message;
use grammers_tl_types as tl;

/// Extensions for making working with updates easier.
pub trait UpdateExt {
    /// Extract the message contained in this update, if any.
    fn message(&self) -> Option<Message>;
}

impl UpdateExt for tl::enums::Update {
    fn message(&self) -> Option<Message> {
        let _message = match self {
            tl::enums::Update::NewMessage(tl::types::UpdateNewMessage { message, .. }) => {
                Some(message)
            }
            tl::enums::Update::NewChannelMessage(tl::types::UpdateNewChannelMessage {
                message,
                ..
            }) => Some(message),
            _ => None,
        };
        todo!()
    }
}
