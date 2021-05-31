// Copyright 2020 - developers of the `grammers` project.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// https://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or https://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.
use grammers_tl_types as tl;
use std::collections::HashMap;

/// In-memory chat cache, mapping peers to their respective access hashes.
pub struct ChatHashCache {
    users: HashMap<i32, i64>,
    channels: HashMap<i32, i64>,
    self_id: Option<i32>,
    self_bot: bool,
}

impl ChatHashCache {
    pub fn new() -> Self {
        Self {
            users: HashMap::new(),
            channels: HashMap::new(),
            self_id: None,
            self_bot: false,
        }
    }

    pub fn self_id(&self) -> i32 {
        self.self_id
            .expect("tried to query self_id before it's known")
    }

    pub fn is_self_bot(&self) -> bool {
        self.self_bot
    }

    pub fn contains_user(&self, user_id: i32) -> bool {
        self.users.contains_key(&user_id)
    }

    pub fn get_input_channel(&self, channel_id: i32) -> Option<tl::enums::InputChannel> {
        self.channels.get(&channel_id).map(|&access_hash| {
            tl::types::InputChannel {
                channel_id,
                access_hash,
            }
            .into()
        })
    }

    pub fn extend(&mut self, users: &[tl::enums::User], chats: &[tl::enums::Chat]) {
        // See https://core.telegram.org/api/min for "issues" with "min constructors".
        use tl::enums::{Chat as C, User as U};
        self.users.extend(users.iter().flat_map(|user| match user {
            U::Empty(_) => None,
            U::User(u) => u.access_hash.and_then(
                |hash| {
                    if u.min {
                        None
                    } else {
                        Some((u.id, hash))
                    }
                },
            ),
        }));
        self.channels
            .extend(chats.iter().flat_map(|chat| match chat {
                C::Empty(_) | C::Chat(_) | C::Forbidden(_) => None,
                C::Channel(c) => c.access_hash.and_then(|hash| {
                    if c.min {
                        None
                    } else {
                        Some((c.id, hash))
                    }
                }),
                C::ChannelForbidden(c) => Some((c.id, c.access_hash)),
            }));
    }
}
