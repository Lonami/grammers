// Copyright 2020 - developers of the `grammers` project.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// https://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or https://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.
use grammers_tl_types as tl;

/// Holds the various different "entities" Telegram knows about in a single place.
#[derive(Clone, Debug)]
pub enum Entity {
    User(tl::types::User),
    Chat(tl::types::Chat),
    Channel(tl::types::Channel),
}

impl Entity {
    pub fn id(&self) -> i32 {
        match self {
            Self::User(user) => user.id,
            Self::Chat(chat) => chat.id,
            Self::Channel(channel) => channel.id,
        }
    }

    pub fn peer(&self) -> tl::enums::Peer {
        use tl::enums::Peer::*;

        match self {
            Self::User(user) => User(tl::types::PeerUser { user_id: user.id }),
            Self::Chat(chat) => Chat(tl::types::PeerChat { chat_id: chat.id }),
            Self::Channel(channel) => Channel(tl::types::PeerChannel {
                channel_id: channel.id,
            }),
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
