// Copyright 2020 - developers of the `grammers` project.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// https://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or https://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.
use grammers_tl_types as tl;

/// A broadcast channel.
///
/// In a broadcast channel, only administrators can broadcast messages to all the subscribers.
/// The rest of users can only join and see messages.
///
/// Broadcast channels and megagroups both are treated as "channels" by Telegram's API, but
/// this variant will always represent a broadcast channel. The only difference between a
/// broadcast channel and a megagroup are the permissions (default, and available).
#[derive(Clone)]
pub struct Channel(tl::types::Channel);

impl Channel {
    pub(crate) fn from_raw(_chat: tl::enums::Chat) -> Self {
        todo!()
    }

    pub(crate) fn to_peer(&self) -> tl::enums::Peer {
        tl::types::PeerChannel {
            channel_id: self.0.id,
        }
        .into()
    }

    pub(crate) fn to_input_peer(&self) -> tl::enums::InputPeer {
        tl::types::InputPeerChannel {
            channel_id: self.0.id,
            access_hash: self.0.access_hash.unwrap_or(0),
        }
        .into()
    }

    /// Return the unique identifier for this channel.
    pub fn id(&self) -> i32 {
        self.0.id
    }

    /// Return the title of this channel.
    pub fn title(&self) -> &str {
        self.0.title.as_str()
    }
}
