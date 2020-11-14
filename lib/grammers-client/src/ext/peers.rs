// Copyright 2020 - developers of the `grammers` project.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// https://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or https://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.
use grammers_tl_types as tl;

pub trait InputPeerExt {
    fn to_chat_id(&self) -> Option<i32>;
    fn to_input_user(&self) -> Option<tl::enums::InputUser>;
    fn to_input_channel(&self) -> Option<tl::enums::InputChannel>;
}

pub trait UserExt {
    fn id(&self) -> i32;
}

impl InputPeerExt for tl::enums::InputPeer {
    fn to_chat_id(&self) -> Option<i32> {
        use tl::enums::InputPeer::*;

        match self {
            Chat(chat) => Some(chat.chat_id),
            // We want to know if Telegram adds more peers, avoid using `_`.
            Empty
            | PeerSelf
            | User(_)
            | UserFromMessage(_)
            | Channel(_)
            | ChannelFromMessage(_) => None,
        }
    }

    fn to_input_user(&self) -> Option<tl::enums::InputUser> {
        use tl::enums::{InputPeer as Peer, InputUser as User};

        match self {
            Peer::Empty => Some(User::Empty),
            Peer::PeerSelf => Some(User::UserSelf),
            Peer::User(user) => Some(
                tl::types::InputUser {
                    user_id: user.user_id,
                    access_hash: user.access_hash,
                }
                .into(),
            ),
            Peer::UserFromMessage(user) => Some(
                tl::types::InputUserFromMessage {
                    peer: user.peer.clone(),
                    msg_id: user.msg_id,
                    user_id: user.user_id,
                }
                .into(),
            ),
            Peer::Chat(_) | Peer::Channel(_) | Peer::ChannelFromMessage(_) => None,
        }
    }

    fn to_input_channel(&self) -> Option<tl::enums::InputChannel> {
        use tl::enums::InputPeer::*;

        match self {
            Empty => Some(tl::enums::InputChannel::Empty),
            Channel(channel) => Some(
                tl::types::InputChannel {
                    channel_id: channel.channel_id,
                    access_hash: channel.access_hash,
                }
                .into(),
            ),
            ChannelFromMessage(channel) => Some(
                tl::types::InputChannelFromMessage {
                    peer: channel.peer.clone(),
                    msg_id: channel.msg_id,
                    channel_id: channel.channel_id,
                }
                .into(),
            ),
            // We want to know if Telegram adds more peers, avoid using `_`.
            PeerSelf | User(_) | UserFromMessage(_) | Chat(_) => None,
        }
    }
}

impl UserExt for tl::enums::User {
    fn id(&self) -> i32 {
        use tl::enums::User::*;

        match self {
            Empty(user) => user.id,
            User(user) => user.id,
        }
    }
}
