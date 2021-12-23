// Copyright 2020 - developers of the `grammers` project.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// https://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or https://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.
use super::{PackedChat, PackedType};
use grammers_tl_types as tl;
use std::collections::HashMap;

/// In-memory chat cache, mapping peers to their respective access hashes.
pub struct ChatHashCache {
    // As far as I've observed, user, chat and channel IDs cannot collide,
    // but it will be an interesting moment if they ever do.
    hash_map: HashMap<i64, (i64, PackedType)>,
    self_id: Option<i64>,
    self_bot: bool,
}

impl ChatHashCache {
    pub fn new(self_user: Option<(i64, bool)>) -> Self {
        Self {
            hash_map: HashMap::new(),
            self_id: self_user.map(|user| user.0),
            self_bot: self_user.map(|user| user.1).unwrap_or(false),
        }
    }

    pub fn self_id(&self) -> i64 {
        self.self_id
            .expect("tried to query self_id before it's known")
    }

    pub fn is_self_bot(&self) -> bool {
        self.self_bot
    }

    pub fn set_self_user(&mut self, user: PackedChat) {
        self.self_bot = match user.ty {
            PackedType::User => false,
            PackedType::Bot => true,
            _ => panic!("tried to set self-user without providing user type"),
        };
        self.self_id = Some(user.id);
    }

    pub fn get(&self, id: i64) -> Option<PackedChat> {
        self.hash_map.get(&id).map(|&(hash, ty)| PackedChat {
            ty,
            id,
            access_hash: Some(hash),
        })
    }

    pub fn extend(&mut self, users: &[tl::enums::User], chats: &[tl::enums::Chat]) {
        // See https://core.telegram.org/api/min for "issues" with "min constructors".
        use tl::enums::{Chat as C, User as U};
        self.hash_map
            .extend(users.iter().flat_map(|user| match user {
                U::Empty(_) => None,
                U::User(u) => u.access_hash.and_then(|hash| {
                    if u.min {
                        None
                    } else {
                        let ty = if u.bot {
                            PackedType::Bot
                        } else {
                            PackedType::User
                        };
                        Some((u.id, (hash, ty)))
                    }
                }),
            }));
        self.hash_map
            .extend(chats.iter().flat_map(|chat| match chat {
                C::Empty(_) | C::Chat(_) | C::Forbidden(_) => None,
                C::Channel(c) => c.access_hash.and_then(|hash| {
                    if c.min {
                        None
                    } else {
                        let ty = if c.megagroup {
                            PackedType::Megagroup
                        } else if c.gigagroup {
                            PackedType::Gigagroup
                        } else {
                            PackedType::Broadcast
                        };
                        Some((c.id, (hash, ty)))
                    }
                }),
                C::ChannelForbidden(c) => {
                    let ty = if c.megagroup {
                        PackedType::Megagroup
                    } else {
                        PackedType::Broadcast
                    };
                    Some((c.id, (c.access_hash, ty)))
                }
            }));
    }
}
