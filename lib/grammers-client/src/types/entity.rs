// Copyright 2020 - developers of the `grammers` project.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// https://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or https://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.
use grammers_tl_types as tl;

// TODO this "maybe borrowed" thing is a bit annoying
// ideally there would be a concrete borrowed type and a concrete owned type?
#[derive(Debug)]
pub enum Entity<'a> {
    User(&'a tl::types::User),
    Chat(&'a tl::types::Chat),
    Channel(&'a tl::types::Channel),
}

impl<'a> Entity<'a> {
    pub fn id(&self) -> i32 {
        match self {
            Self::User(user) => user.id,
            Self::Chat(chat) => chat.id,
            Self::Channel(channel) => channel.id,
        }
    }

    pub fn to_input_peer(&self) -> tl::enums::InputPeer {
        match self {
            Self::User(user) => tl::types::InputPeerUser {
                user_id: user.id,
                access_hash: user.access_hash.unwrap_or(0),
            }
            .into(),
            Self::Chat(chat) => tl::types::InputPeerChat { chat_id: chat.id }.into(),
            Self::Channel(channel) => tl::types::InputPeerChannel {
                channel_id: channel.id,
                access_hash: channel.access_hash.unwrap_or(0),
            }
            .into(),
        }
    }

    pub fn name(&self) -> &str {
        match self {
            Self::User(user) => {
                if let Some(name) = &user.first_name {
                    name
                } else {
                    "Deleted Account"
                }
            }
            Self::Chat(chat) => &chat.title,
            Self::Channel(channel) => &channel.title,
        }
    }
}
