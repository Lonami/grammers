// Copyright 2020 - developers of the `grammers` project.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// https://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or https://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

use chrono::{DateTime, Utc};
use grammers_session::types::PeerId;
use grammers_tl_types as tl;

use super::{Peer, PeerMap, Permissions, Restrictions};
use crate::{peer::User, utils};

/// Chat participant with default permissions.
#[derive(Clone, Debug, PartialEq)]
pub struct Normal {
    date: i32,
    inviter_id: Option<i64>,
}

/// Chat participant that created the chat itself.
#[derive(Clone, Debug, PartialEq)]
pub struct Creator {
    permissions: Permissions,
    rank: Option<String>,
}

/// Chat participant promoted to administrator.
#[derive(Clone, Debug, PartialEq)]
pub struct Admin {
    can_edit: bool,
    inviter_id: Option<i64>,
    promoted_by: Option<i64>,
    date: i32,
    permissions: Permissions,
    rank: Option<String>,
}

/// Chat participant demoted to have restrictions.
#[derive(Clone, Debug, PartialEq)]
pub struct Banned {
    left: bool,
    kicked_by: i64,
    date: i32,
    restrictions: Restrictions,
}

/// Chat participant no longer present in the chat.
#[derive(Clone, Debug, PartialEq)]
pub struct Left {}

/// Participant role within a group or channel.
#[derive(Clone, Debug, PartialEq)]
#[non_exhaustive]
pub enum Role {
    User(Normal),
    Creator(Creator),
    Admin(Admin),
    Banned(Banned),
    Left(Left),
}

/// User and their role within the group or channel.
#[derive(Clone, Debug)]
#[non_exhaustive]
pub struct Participant {
    pub user: User,
    pub role: Role,
}

impl Normal {
    /// Date when the participant joined.
    pub fn date(&self) -> DateTime<Utc> {
        utils::date(self.date)
    }

    /// Identifier of the person that invited the participant into the chat, if known.
    pub fn inviter_id(&self) -> Option<PeerId> {
        self.inviter_id.map(PeerId::user)
    }
}

impl Creator {
    /// Permissions this administrator has in the chat.
    pub fn permissions(&self) -> &Permissions {
        &self.permissions
    }

    /// Custom administrator title.
    pub fn rank(&self) -> Option<&str> {
        self.rank.as_deref()
    }
}

impl Admin {
    pub fn can_edit(&self) -> bool {
        self.can_edit
    }

    /// Identifier of the person that invited the participant into the chat, if known.
    pub fn inviter_id(&self) -> Option<PeerId> {
        self.inviter_id.map(PeerId::user)
    }

    /// Identifier of the person that promoted the participant, if known.
    pub fn promoted_by(&self) -> Option<PeerId> {
        self.promoted_by.map(PeerId::user)
    }

    pub fn date(&self) -> DateTime<Utc> {
        utils::date(self.date)
    }

    /// Permissions this administrator has in the chat.
    pub fn permissions(&self) -> &Permissions {
        &self.permissions
    }

    /// Custom administrator title.
    pub fn rank(&self) -> Option<&str> {
        self.rank.as_deref()
    }
}

impl Banned {
    pub fn left(&self) -> bool {
        self.left
    }

    /// Identifier of the person that kicked the participant from the chat.
    pub fn kicked_by(&self) -> PeerId {
        PeerId::user(self.kicked_by)
    }

    pub fn date(&self) -> DateTime<Utc> {
        utils::date(self.date)
    }

    /// Restrictions this administrator has in the chat.
    pub fn restrictions(&self) -> &Restrictions {
        &self.restrictions
    }
}

impl Participant {
    pub(crate) fn from_raw_channel(
        peers: &mut PeerMap,
        participant: tl::enums::ChannelParticipant,
    ) -> Self {
        use tl::enums::ChannelParticipant as P;

        match participant {
            P::Participant(p) => Self {
                user: peers.take_user(p.user_id).unwrap(),
                role: Role::User(Normal {
                    date: p.date,
                    inviter_id: None,
                }),
            },
            P::ParticipantSelf(p) => Self {
                user: peers.take_user(p.user_id).unwrap(),
                role: Role::User(Normal {
                    date: p.date,
                    inviter_id: Some(p.inviter_id),
                }),
            },
            P::Creator(p) => Self {
                user: peers.take_user(p.user_id).unwrap(),
                role: Role::Creator(Creator {
                    permissions: Permissions::from_raw(p.admin_rights.into()),
                    rank: p.rank,
                }),
            },
            P::Admin(p) => Self {
                user: peers.take_user(p.user_id).unwrap(),
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
                user: match peers.take(PeerId::from(p.peer.clone())).unwrap() {
                    Peer::User(user) => user,
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
                user: match peers.take(PeerId::from(p.peer.clone())).unwrap() {
                    Peer::User(user) => user,
                    _ => todo!("figure out how to deal with non-user leaving"),
                },
                role: Role::Left(Left {}),
            },
        }
    }

    pub(crate) fn from_raw_chat(
        peers: &mut PeerMap,
        participant: tl::enums::ChatParticipant,
    ) -> Self {
        use tl::enums::ChatParticipant as P;

        match participant {
            P::Participant(p) => Self {
                user: peers.take_user(p.user_id).unwrap(),
                role: Role::User(Normal {
                    date: p.date,
                    inviter_id: Some(p.inviter_id),
                }),
            },
            P::Creator(p) => Self {
                user: peers.take_user(p.user_id).unwrap(),
                role: Role::Creator(Creator {
                    permissions: Permissions::new_full(),
                    rank: None,
                }),
            },
            P::Admin(p) => Self {
                user: peers.take_user(p.user_id).unwrap(),
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
