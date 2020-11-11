// Copyright 2020 - developers of the `grammers` project.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// https://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or https://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.
use grammers_tl_types as tl;

/// Extensions for making working with messages easier.
pub trait MessageExt {
    /// Get the `Peer` chat where this message was sent to.
    fn chat(&self) -> tl::enums::Peer;
    /// Get the optional reply to message id
    fn reply_to_message_id(&self) -> Option<i32>;
    /// Get the id of the message's sender
    fn from_id(&self) -> Option<i32>;
}

impl MessageExt for tl::types::Message {
    fn chat(&self) -> tl::enums::Peer {
        self.peer_id.clone()
    }

    fn reply_to_message_id(&self) -> Option<i32> {
        return if let Some(tl::enums::MessageReplyHeader::Header(m)) = &self.reply_to {
            Some(m.reply_to_msg_id)
        } else {
            None
        };
    }

    fn from_id(&self) -> Option<i32> {
        return if let Some(tl::enums::Peer::User(usr)) = &self.from_id {
            Some(usr.user_id)
        } else {
            None
        };
    }
}
