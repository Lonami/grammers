// Copyright 2020 - developers of the `grammers` project.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// https://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or https://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

use grammers_session::State;
use grammers_tl_types as tl;

/// Occurs whenever a message is deleted.
///
/// When `MessageDeletion#channel_id` is Some, it means the message was deleted
/// from a channel.
#[derive(Debug, Clone)]
pub struct MessageDeletion {
    pub raw: tl::enums::Update,
    pub state: State,
}

impl MessageDeletion {
    /// Returns the channel ID if the message was deleted from a channel.
    pub fn channel_id(&self) -> Option<i64> {
        match &self.raw {
            tl::enums::Update::DeleteMessages(_) => None,
            tl::enums::Update::DeleteChannelMessages(update) => Some(update.channel_id),
            _ => unreachable!(),
        }
    }

    /// Returns the slice of message IDs that was deleted.
    pub fn messages(&self) -> &[i32] {
        match &self.raw {
            tl::enums::Update::DeleteMessages(update) => update.messages.as_slice(),
            tl::enums::Update::DeleteChannelMessages(update) => update.messages.as_slice(),
            _ => unreachable!(),
        }
    }

    /// Gain ownership of underlying Vec of message IDs that was deleted.
    pub fn into_messages(self) -> Vec<i32> {
        match self.raw {
            tl::enums::Update::DeleteMessages(update) => update.messages,
            tl::enums::Update::DeleteChannelMessages(update) => update.messages,
            _ => unreachable!(),
        }
    }
}
