// Copyright 2020 - developers of the `grammers` project.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// https://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or https://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.
use grammers_tl_types as tl;
use std::fmt;

/// A broadcast channel.
///
/// In a broadcast channel, only administrators can broadcast messages to all the subscribers.
/// The rest of users can only join and see messages.
///
/// Broadcast channels and megagroups both are treated as "channels" by Telegram's API, but
/// this variant will always represent a broadcast channel. The only difference between a
/// broadcast channel and a megagroup are the permissions (default, and available).
#[derive(Clone)]
pub struct Channel(pub(crate) tl::types::Channel);

impl fmt::Debug for Channel {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

impl Channel {
    pub(crate) fn from_raw(chat: tl::enums::Chat) -> Self {
        use tl::enums::Chat as C;

        match chat {
            C::Empty(_) | C::Chat(_) | C::Forbidden(_) => panic!("cannot create from group chat"),
            C::Channel(channel) => {
                if channel.broadcast {
                    Self(channel)
                } else {
                    panic!("tried to create broadcast channel from megagroup");
                }
            }
            C::ChannelForbidden(channel) => {
                if channel.broadcast {
                    // TODO store until_date
                    Self(tl::types::Channel {
                        creator: false,
                        left: false,
                        broadcast: channel.broadcast,
                        verified: false,
                        megagroup: channel.megagroup,
                        restricted: false,
                        signatures: false,
                        min: false,
                        scam: false,
                        has_link: false,
                        has_geo: false,
                        slowmode_enabled: false,
                        call_active: false,
                        call_not_empty: false,
                        fake: false,
                        gigagroup: false,
                        id: channel.id,
                        access_hash: Some(channel.access_hash),
                        title: channel.title,
                        username: None,
                        photo: tl::enums::ChatPhoto::Empty,
                        date: 0,
                        version: 0,
                        restriction_reason: None,
                        admin_rights: None,
                        banned_rights: None,
                        default_banned_rights: None,
                        participants_count: None,
                    })
                } else {
                    panic!("tried to create broadcast channel from megagroup");
                }
            }
        }
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

    pub(crate) fn to_input(&self) -> tl::enums::InputChannel {
        tl::types::InputChannel {
            channel_id: self.0.id,
            access_hash: self.0.access_hash.unwrap_or(0),
        }
        .into()
    }

    /// Return the unique identifier for this channel.
    pub fn id(&self) -> i32 {
        self.0.id
    }

    pub(crate) fn access_hash(&self) -> Option<i64> {
        self.0.access_hash
    }

    /// Return the title of this channel.
    pub fn title(&self) -> &str {
        self.0.title.as_str()
    }
}
