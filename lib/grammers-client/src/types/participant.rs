// Copyright 2020 - developers of the `grammers` project.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// https://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or https://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.
use super::{Chat, ChatMap, Permissions, Restrictions};
use crate::utils;
use grammers_tl_types as tl;

#[derive(Clone, Debug, PartialEq)]
pub struct Normal {
    date: i32,
    inviter_id: Option<i32>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Creator {
    permissions: Permissions,
    rank: Option<String>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Admin {
    can_edit: bool,
    inviter_id: Option<i32>,
    promoted_by: Option<i32>,
    date: i32,
    permissions: Permissions,
    rank: Option<String>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Banned {
    left: bool,
    kicked_by: i32,
    date: i32,
    restrictions: Restrictions,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Left {}

#[derive(Clone, Debug, PartialEq)]
#[non_exhaustive]
pub enum Role {
    User(Normal),
    Creator(Creator),
    Admin(Admin),
    Banned(Banned),
    Left(Left),
}

#[derive(Clone, Debug)]
#[non_exhaustive]
pub struct Participant {
    pub user: crate::types::User,
    pub role: Role,
}

impl Normal {
    pub fn date(&self) -> utils::Date {
        utils::date(self.date)
    }

    pub fn inviter_id(&self) -> Option<i32> {
        self.inviter_id
    }
}

impl Creator {
    pub fn permissions(&self) -> &Permissions {
        &self.permissions
    }

    pub fn rank(&self) -> Option<&str> {
        self.rank.as_deref()
    }
}

impl Admin {
    pub fn can_edit(&self) -> bool {
        self.can_edit
    }

    pub fn inviter_id(&self) -> Option<i32> {
        self.inviter_id
    }

    pub fn promoted_by(&self) -> Option<i32> {
        self.promoted_by
    }

    pub fn date(&self) -> utils::Date {
        utils::date(self.date)
    }

    pub fn permissions(&self) -> &Permissions {
        &self.permissions
    }

    pub fn rank(&self) -> Option<&str> {
        self.rank.as_deref()
    }
}

impl Banned {
    pub fn left(&self) -> bool {
        self.left
    }

    pub fn kicked_by(&self) -> i32 {
        self.kicked_by
    }

    pub fn date(&self) -> utils::Date {
        utils::date(self.date)
    }

    pub fn restrictions(&self) -> &Restrictions {
        &self.restrictions
    }
}

impl Participant {
    pub(crate) fn from_raw_channel(
        chats: &mut ChatMap,
        participant: tl::enums::ChannelParticipant,
    ) -> Self {
        use tl::enums::ChannelParticipant as P;

        match participant {
            P::Participant(p) => Self {
                user: chats.remove_user(p.user_id).unwrap(),
                role: Role::User(Normal {
                    date: p.date,
                    inviter_id: None,
                }),
            },
            P::ParticipantSelf(p) => Self {
                user: chats.remove_user(p.user_id).unwrap(),
                role: Role::User(Normal {
                    date: p.date,
                    inviter_id: Some(p.inviter_id),
                }),
            },
            P::Creator(p) => Self {
                user: chats.remove_user(p.user_id).unwrap(),
                role: Role::Creator(Creator {
                    permissions: Permissions::from_raw(p.admin_rights.into()),
                    rank: p.rank,
                }),
            },
            P::Admin(p) => Self {
                user: chats.remove_user(p.user_id).unwrap(),
                role: Role::Admin(Admin {
                    can_edit: p.can_edit,
                    inviter_id: p.inviter_id,
                    promoted_by: Some(p.promoted_by),
                    date: p.date,
                    permissions: Permissions::from_raw(p.admin_rights.into()),
                    rank: p.rank,
                }),
            },
            P::Banned(p) => Self {
                user: match chats.remove(&p.peer).unwrap() {
                    Chat::User(user) => user,
                    _ => todo!("figure out how to deal with non-user being banned"),
                },
                role: Role::Banned(Banned {
                    left: p.left,
                    kicked_by: p.kicked_by,
                    date: p.date,
                    restrictions: Restrictions::from_raw(p.banned_rights.into()),
                }),
            },
            P::Left(p) => Self {
                user: match chats.remove(&p.peer).unwrap() {
                    Chat::User(user) => user,
                    _ => todo!("figure out how to deal with non-user leaving"),
                },
                role: Role::Left(Left {}),
            },
        }
    }

    pub(crate) fn from_raw_chat(
        chats: &mut ChatMap,
        participant: tl::enums::ChatParticipant,
    ) -> Self {
        use tl::enums::ChatParticipant as P;

        match participant {
            P::Participant(p) => Self {
                user: chats.remove_user(p.user_id).unwrap(),
                role: Role::User(Normal {
                    date: p.date,
                    inviter_id: Some(p.inviter_id),
                }),
            },
            P::Creator(p) => Self {
                user: chats.remove_user(p.user_id).unwrap(),
                role: Role::Creator(Creator {
                    permissions: Permissions::new_full(),
                    rank: None,
                }),
            },
            P::Admin(p) => Self {
                user: chats.remove_user(p.user_id).unwrap(),
                role: Role::Admin(Admin {
                    can_edit: true,
                    inviter_id: Some(p.inviter_id),
                    promoted_by: None,
                    date: p.date,
                    permissions: Permissions::new_full(),
                    rank: None,
                }),
            },
        }
    }
}
