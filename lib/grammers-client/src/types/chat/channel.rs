// Copyright 2020 - developers of the `grammers` project.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// https://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or https://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.
use grammers_session::{PackedChat, PackedType};
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
pub struct Channel {
    pub raw: tl::types::Channel,
}

impl fmt::Debug for Channel {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.raw.fmt(f)
    }
}

impl Channel {
    pub fn from_raw(chat: tl::enums::Chat) -> Self {
        use tl::enums::Chat as C;

        match chat {
            C::Empty(_) | C::Chat(_) | C::Forbidden(_) => panic!("cannot create from group chat"),
            C::Channel(channel) => {
                if channel.broadcast {
                    Self { raw: channel }
                } else {
                    panic!("tried to create broadcast channel from megagroup");
                }
            }
            C::ChannelForbidden(channel) => {
                if channel.broadcast {
                    // TODO store until_date
                    Self {
                        raw: tl::types::Channel {
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
                            noforwards: false,
                            join_request: false,
                            forum: false,
                            stories_hidden: false,
                            stories_hidden_min: false,
                            stories_unavailable: true,
                            signature_profiles: false,
                            join_to_send: false,
                            id: channel.id,
                            access_hash: Some(channel.access_hash),
                            title: channel.title,
                            username: None,
                            photo: tl::enums::ChatPhoto::Empty,
                            date: 0,
                            restriction_reason: None,
                            admin_rights: None,
                            banned_rights: None,
                            default_banned_rights: None,
                            participants_count: None,
                            usernames: None,
                            stories_max_id: None,
                            color: None,
                            profile_color: None,
                            emoji_status: None,
                            level: None,
                            subscription_until_date: None,
                        },
                    }
                } else {
                    panic!("tried to create broadcast channel from megagroup");
                }
            }
        }
    }

    /// Return the unique identifier for this channel.
    pub fn id(&self) -> i64 {
        self.raw.id
    }

    /// Pack this channel into a smaller representation that can be loaded later.
    pub fn pack(&self) -> PackedChat {
        PackedChat {
            ty: if self.raw.gigagroup {
                PackedType::Gigagroup
            } else {
                PackedType::Broadcast
            },
            id: self.id(),
            access_hash: self.raw.access_hash,
        }
    }

    /// Return the title of this channel.
    pub fn title(&self) -> &str {
        self.raw.title.as_str()
    }

    /// Return the public @username of this channel, if any.
    ///
    /// The returned username does not contain the "@" prefix.
    ///
    /// Outside of the application, people may link to this user with one of Telegram's URLs, such
    /// as https://t.me/username.
    pub fn username(&self) -> Option<&str> {
        self.raw.username.as_deref()
    }

    /// Return collectible usernames of this chat, if any.
    ///
    /// The returned usernames do not contain the "@" prefix.
    ///
    /// Outside of the application, people may link to this user with one of its username, such
    /// as https://t.me/username.
    pub fn usernames(&self) -> Vec<&str> {
        self.raw
            .usernames
            .as_deref()
            .map_or(Vec::new(), |usernames| {
                usernames
                    .iter()
                    .map(|username| match username {
                        tl::enums::Username::Username(username) => username.username.as_ref(),
                    })
                    .collect()
            })
    }

    /// Return the photo of this channel, if any.
    pub fn photo(&self) -> Option<&tl::types::ChatPhoto> {
        match &self.raw.photo {
            tl::enums::ChatPhoto::Empty => None,
            tl::enums::ChatPhoto::Photo(photo) => Some(photo),
        }
    }

    /// Return the permissions of the logged-in user in this channel.
    pub fn admin_rights(&self) -> Option<&tl::types::ChatAdminRights> {
        match &self.raw.admin_rights {
            Some(tl::enums::ChatAdminRights::Rights(rights)) => Some(rights),
            None if self.raw.creator => Some(&tl::types::ChatAdminRights {
                add_admins: true,
                other: true,
                change_info: true,
                post_messages: true,
                anonymous: false,
                ban_users: true,
                delete_messages: true,
                edit_messages: true,
                invite_users: true,
                manage_call: true,
                pin_messages: true,
                manage_topics: true,
                post_stories: true,
                edit_stories: true,
                delete_stories: true,
            }),
            None => None,
        }
    }
}

impl From<Channel> for PackedChat {
    fn from(chat: Channel) -> Self {
        chat.pack()
    }
}

impl From<&Channel> for PackedChat {
    fn from(chat: &Channel) -> Self {
        chat.pack()
    }
}
