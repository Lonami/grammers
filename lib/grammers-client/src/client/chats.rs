// Copyright 2020 - developers of the `grammers` project.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// https://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or https://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

//! Methods related to chats and entities.

use super::{Client, ClientHandle};
use crate::ext::{InputPeerExt, UserExt};
use crate::types::IterBuffer;
pub use grammers_mtsender::{AuthorizationError, InvocationError};
use grammers_tl_types as tl;
use std::collections::{HashMap, VecDeque};
use std::convert::TryInto;

const MAX_LIMIT: usize = 200;

fn full_rights() -> tl::types::ChatAdminRights {
    tl::types::ChatAdminRights {
        change_info: true,
        post_messages: true,
        edit_messages: true,
        delete_messages: true,
        ban_users: true,
        invite_users: true,
        pin_messages: true,
        add_admins: true,
        anonymous: true,
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum Role {
    User {
        date: i32,
        inviter_id: Option<i32>,
    },
    Creator {
        admin_rights: tl::types::ChatAdminRights,
        rank: Option<String>,
    },
    Admin {
        can_edit: bool,
        inviter_id: Option<i32>,
        promoted_by: Option<i32>,
        date: i32,
        admin_rights: tl::types::ChatAdminRights,
        rank: Option<String>,
    },
    Banned {
        left: bool,
        kicked_by: i32,
        date: i32,
        banned_rights: tl::types::ChatBannedRights,
    },
}

#[derive(Clone, Debug)]
pub struct Participant {
    pub user: tl::types::User,
    pub role: Role,
}

pub enum ParticipantIter {
    Empty,
    Chat {
        client: ClientHandle,
        chat_id: i32,
        buffer: VecDeque<Participant>,
        total: Option<usize>,
    },
    Channel(IterBuffer<tl::functions::channels::GetParticipants, Participant>),
}

impl ParticipantIter {
    fn new(client: &ClientHandle, chat: &tl::enums::InputPeer) -> Self {
        if let Some(channel) = chat.to_input_channel() {
            Self::Channel(IterBuffer::from_request(
                client,
                MAX_LIMIT,
                tl::functions::channels::GetParticipants {
                    channel,
                    filter: tl::enums::ChannelParticipantsFilter::ChannelParticipantsRecent,
                    offset: 0,
                    limit: 0,
                    hash: 0,
                },
            ))
        } else if let Some(chat_id) = chat.to_chat_id() {
            Self::Chat {
                client: client.clone(),
                chat_id,
                buffer: VecDeque::new(),
                total: None,
            }
        } else {
            Self::Empty
        }
    }

    /// Determines how many participants there are in total.
    ///
    /// This only performs a network call if `next` has not been called before.
    pub async fn total(&mut self) -> Result<usize, InvocationError> {
        match self {
            Self::Empty => Ok(0),
            Self::Chat { total, .. } => {
                if let Some(total) = total {
                    Ok(*total)
                } else {
                    self.fill_buffer().await
                }
            }
            Self::Channel(iter) => {
                if let Some(total) = iter.total {
                    Ok(total)
                } else {
                    self.fill_buffer().await
                }
            }
        }
    }

    /// Fills the buffer, and returns the total count.
    async fn fill_buffer(&mut self) -> Result<usize, InvocationError> {
        match self {
            Self::Empty => Ok(0),
            Self::Chat {
                client,
                chat_id,
                buffer,
                total,
            } => {
                assert!(buffer.is_empty());
                let tl::enums::messages::ChatFull::Full(full) = client
                    .invoke(&tl::functions::messages::GetFullChat { chat_id: *chat_id })
                    .await?;

                let chat = match full.full_chat {
                    tl::enums::ChatFull::Full(chat) => chat,
                    tl::enums::ChatFull::ChannelFull(_) => panic!(
                        "API returned ChannelFull even though messages::GetFullChat was used"
                    ),
                };

                let participants = match chat.participants {
                    tl::enums::ChatParticipants::Forbidden(c) => {
                        // TODO consider filling the buffer, even if just with ourself
                        return Ok(if c.self_participant.is_some() { 1 } else { 0 });
                    }
                    tl::enums::ChatParticipants::Participants(c) => c.participants,
                };

                let mut users = full
                    .users
                    .into_iter()
                    .map(|user| (user.id(), user))
                    .collect::<HashMap<_, _>>();

                use tl::enums::ChatParticipant as ChPart;

                buffer.extend(participants.into_iter().filter_map(|participant| {
                    let (user_id, role) = match participant {
                        ChPart::Participant(p) => (
                            p.user_id,
                            Role::User {
                                date: p.date,
                                inviter_id: Some(p.inviter_id),
                            },
                        ),
                        ChPart::Creator(p) => (
                            p.user_id,
                            Role::Creator {
                                admin_rights: full_rights(),
                                rank: None,
                            },
                        ),
                        ChPart::Admin(p) => (
                            p.user_id,
                            Role::Admin {
                                can_edit: true,
                                inviter_id: Some(p.inviter_id),
                                promoted_by: None,
                                date: p.date,
                                admin_rights: full_rights(),
                                rank: None,
                            },
                        ),
                    };

                    users.remove(&user_id).and_then(|user| {
                        Some(Participant {
                            user: user.try_into().ok()?,
                            role,
                        })
                    })
                }));

                *total = Some(buffer.len());
                Ok(buffer.len())
            }
            Self::Channel(iter) => {
                assert!(iter.buffer.is_empty());
                use tl::enums::channels::ChannelParticipants::*;

                iter.request.limit = iter.determine_limit(MAX_LIMIT);
                let (count, participants, users) = match iter.client.invoke(&iter.request).await? {
                    Participants(p) => (p.count, p.participants, p.users),
                    NotModified => panic!("API returned Dialogs::NotModified even though hash = 0"),
                };
                iter.last_chunk = participants.len() < iter.request.limit as usize;

                // Don't bother updating offsets if this is the last time stuff has to be fetched.
                if !iter.last_chunk && !iter.buffer.is_empty() {
                    iter.request.offset += participants.len() as i32;
                }

                let mut users = users
                    .into_iter()
                    .map(|user| (user.id(), user))
                    .collect::<HashMap<_, _>>();

                use tl::enums::ChannelParticipant as ChPart;

                iter.buffer
                    .extend(participants.into_iter().filter_map(|participant| {
                        let (user_id, role) = match participant {
                            ChPart::Participant(p) => (
                                p.user_id,
                                Role::User {
                                    date: p.date,
                                    inviter_id: None,
                                },
                            ),
                            ChPart::ParticipantSelf(p) => (
                                p.user_id,
                                Role::User {
                                    date: p.date,
                                    inviter_id: Some(p.inviter_id),
                                },
                            ),
                            ChPart::Creator(p) => (
                                p.user_id,
                                Role::Creator {
                                    admin_rights: p.admin_rights.into(),
                                    rank: p.rank,
                                },
                            ),
                            ChPart::Admin(p) => (
                                p.user_id,
                                Role::Admin {
                                    can_edit: p.can_edit,
                                    inviter_id: p.inviter_id,
                                    promoted_by: Some(p.promoted_by),
                                    date: p.date,
                                    admin_rights: p.admin_rights.into(),
                                    rank: p.rank,
                                },
                            ),
                            ChPart::Banned(p) => (
                                p.user_id,
                                Role::Banned {
                                    left: p.left,
                                    kicked_by: p.kicked_by,
                                    date: p.date,
                                    banned_rights: p.banned_rights.into(),
                                },
                            ),
                        };
                        users.remove(&user_id).and_then(|user| {
                            Some(Participant {
                                user: user.try_into().ok()?,
                                role,
                            })
                        })
                    }));

                iter.total = Some(count as usize);
                Ok(count as usize)
            }
        }
    }

    /// Return the next `Participant` from the internal buffer, filling the buffer previously if
    /// it's empty.
    ///
    /// Returns `None` if the `limit` is reached or there are no participants left.
    pub async fn next(&mut self) -> Result<Option<Participant>, InvocationError> {
        // Need to split the `match` because `fill_buffer()` borrows mutably.
        match self {
            Self::Empty => {}
            Self::Chat { buffer, .. } => {
                if buffer.is_empty() {
                    self.fill_buffer().await?;
                }
            }
            Self::Channel(iter) => {
                if let Some(result) = iter.next_raw() {
                    return result;
                }
                self.fill_buffer().await?;
            }
        }

        match self {
            Self::Empty => Ok(None),
            Self::Chat { buffer, .. } => {
                let result = buffer.pop_front();
                if buffer.is_empty() {
                    *self = Self::Empty;
                }
                Ok(result)
            }
            Self::Channel(iter) => Ok(iter.pop_item()),
        }
    }
}

impl Client {
    pub(crate) fn user_id(&self) -> Option<i32> {
        // TODO actually use the user id saved in the session from login
        Some(0)
    }
}

impl ClientHandle {
    /// Resolves a username into the user that owns it, if any.
    pub async fn resolve_username(
        &mut self,
        username: &str,
    ) -> Result<Option<tl::types::User>, InvocationError> {
        let tl::enums::contacts::ResolvedPeer::Peer(tl::types::contacts::ResolvedPeer {
            peer,
            users,
            ..
        }) = self
            .invoke(&tl::functions::contacts::ResolveUsername {
                username: username.into(),
            })
            .await?;

        Ok(match peer {
            tl::enums::Peer::User(tl::types::PeerUser { user_id }) => {
                users.into_iter().find_map(|user| match user {
                    tl::enums::User::User(user) if user.id == user_id => Some(user),
                    tl::enums::User::User(_) | tl::enums::User::Empty(_) => None,
                })
            }
            tl::enums::Peer::Chat(_) | tl::enums::Peer::Channel(_) => None,
        })
    }

    /// Fetch full information about the currently logged-in user.
    pub async fn get_me(&mut self) -> Result<tl::types::User, InvocationError> {
        let mut res = self
            .invoke(&tl::functions::users::GetUsers {
                id: vec![tl::enums::InputUser::UserSelf],
            })
            .await?;

        if res.len() != 1 {
            panic!("fetching only one user should exactly return one user");
        }

        match res.pop().unwrap() {
            tl::enums::User::User(user) => Ok(user),
            tl::enums::User::Empty(_) => panic!("should not get empty user when fetching self"),
        }
    }

    /// Iterate over the participants of a chat.
    ///
    /// The participants are returned in no particular order.
    ///
    /// When used to iterate the participants of "user", the iterator won't produce values.
    pub fn iter_participants(&self, chat: &tl::enums::InputPeer) -> ParticipantIter {
        ParticipantIter::new(self, chat)
    }
}
