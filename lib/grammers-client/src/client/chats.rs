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
use crate::types::{AdminRightsBuilder, BannedRightsBuilder, Entity, IterBuffer, Message};
pub use grammers_mtsender::{AuthorizationError, InvocationError};
use grammers_tl_types as tl;
use std::collections::{HashMap, VecDeque};
use std::convert::TryInto;
use std::time::Duration;

const MAX_PARTICIPANT_LIMIT: usize = 200;
const MAX_PHOTO_LIMIT: usize = 100;
const KICK_BAN_DURATION: i32 = 60; // in seconds, in case the second request fails

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
        manage_call: true,
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
    Left,
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
                MAX_PARTICIPANT_LIMIT,
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

                iter.request.limit = iter.determine_limit(MAX_PARTICIPANT_LIMIT);
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
                            ChPart::Left(p) => (p.user_id, Role::Left),
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

pub enum ProfilePhotoIter {
    User(IterBuffer<tl::functions::photos::GetUserPhotos, tl::types::Photo>),
    Chat(IterBuffer<tl::functions::messages::Search, Message>),
}

impl ProfilePhotoIter {
    fn new(client: &ClientHandle, chat: &tl::enums::InputPeer) -> Self {
        if let Some(user_id) = chat.to_input_user() {
            Self::User(IterBuffer::from_request(
                client,
                MAX_PHOTO_LIMIT,
                tl::functions::photos::GetUserPhotos {
                    user_id,
                    offset: 0,
                    max_id: 0,
                    limit: 0,
                },
            ))
        } else {
            Self::Chat(
                client
                    .search_messages(chat)
                    .filter(tl::enums::MessagesFilter::InputMessagesFilterChatPhotos),
            )
        }
    }

    /// Determines how many profile photos there are in total.
    ///
    /// This only performs a network call if `next` has not been called before.
    pub async fn total(&mut self) -> Result<usize, InvocationError> {
        match self {
            Self::User(iter) => {
                if let Some(total) = iter.total {
                    Ok(total)
                } else {
                    self.fill_buffer().await
                }
            }
            Self::Chat(iter) => iter.total().await,
        }
    }

    /// Fills the buffer, and returns the total count.
    async fn fill_buffer(&mut self) -> Result<usize, InvocationError> {
        match self {
            Self::User(iter) => {
                use tl::enums::photos::Photos;

                iter.request.limit = iter.determine_limit(MAX_PHOTO_LIMIT);
                let (total, photos) = match iter.client.invoke(&iter.request).await? {
                    Photos::Photos(p) => {
                        iter.last_chunk = true;
                        iter.total = Some(p.photos.len());
                        (p.photos.len(), p.photos)
                    }
                    Photos::Slice(p) => {
                        iter.last_chunk = p.photos.len() < iter.request.limit as usize;
                        iter.total = Some(p.count as usize);
                        (p.count as usize, p.photos)
                    }
                };

                // Don't bother updating offsets if this is the last time stuff has to be fetched.
                if !iter.last_chunk && !iter.buffer.is_empty() {
                    iter.request.offset += photos.len() as i32;
                }

                iter.buffer
                    .extend(photos.into_iter().flat_map(|photo| photo.try_into().ok()));
                Ok(total)
            }
            Self::Chat(_) => panic!("fill_buffer should not be called for Chat variant"),
        }
    }

    /// Return the next photo from the internal buffer, filling the buffer previously if it's
    /// empty.
    ///
    /// Returns `None` if the `limit` is reached or there are no photos left.
    pub async fn next(&mut self) -> Result<Option<tl::types::Photo>, InvocationError> {
        use tl::enums::{MessageAction, Photo};

        // Need to split the `match` because `fill_buffer()` borrows mutably.
        match self {
            Self::User(iter) => {
                if let Some(result) = iter.next_raw() {
                    return result;
                }
                self.fill_buffer().await?;
            }
            Self::Chat(iter) => {
                while let Some(message) = iter.next().await? {
                    if let Some(MessageAction::ChatEditPhoto(
                        tl::types::MessageActionChatEditPhoto {
                            photo: Photo::Photo(photo),
                        },
                    )) = message.action
                    {
                        return Ok(Some(photo));
                    } else {
                        continue;
                    }
                }
            }
        }

        match self {
            Self::User(iter) => Ok(iter.pop_item()),
            Self::Chat(_) => Ok(None),
        }
    }
}

impl Client {
    pub(crate) fn user_id(&self) -> Option<i32> {
        // TODO actually use the user id saved in the session from login
        Some(0)
    }
}

/// Method implementations related to dealing with chats or other users.
impl ClientHandle {
    /// Resolves a username into the user that owns it, if any.
    ///
    /// Note that this method is expensive to call, and can quickly cause long flood waits.
    ///
    /// # Examples
    ///
    /// ```
    /// # async fn f(mut client: grammers_client::ClientHandle) -> Result<(), Box<dyn std::error::Error>> {
    /// if let Some(entity) = client.resolve_username("username").await? {
    ///     println!("Found entity!: {:?}", entity);
    /// }
    /// # Ok(())
    /// # }
    /// ```
    pub async fn resolve_username(
        &mut self,
        username: &str,
    ) -> Result<Option<Entity>, InvocationError> {
        let tl::enums::contacts::ResolvedPeer::Peer(tl::types::contacts::ResolvedPeer {
            peer,
            users,
            chats,
        }) = self
            .invoke(&tl::functions::contacts::ResolveUsername {
                username: username.into(),
            })
            .await?;

        Ok(match peer {
            tl::enums::Peer::User(tl::types::PeerUser { user_id }) => {
                users.into_iter().find_map(|user| match user {
                    tl::enums::User::User(user) if user.id == user_id => Some(Entity::User(user)),
                    tl::enums::User::User(_) | tl::enums::User::Empty(_) => None,
                })
            }
            tl::enums::Peer::Chat(tl::types::PeerChat { chat_id }) => {
                chats.into_iter().find_map(|chat| match chat {
                    tl::enums::Chat::Chat(c) if c.id == chat_id => Some(Entity::Chat(c)),
                    _ => None,
                })
            }
            tl::enums::Peer::Channel(tl::types::PeerChannel { channel_id }) => {
                chats.into_iter().find_map(|chan| match chan {
                    tl::enums::Chat::Channel(ch) if ch.id == channel_id => {
                        Some(Entity::Channel(ch))
                    }
                    _ => None,
                })
            }
        })
    }

    /// Fetch full information about the currently logged-in user.
    ///
    /// Although this method is cheap to call, you might want to cache the results somewhere.
    ///
    /// # Examples
    ///
    /// ```
    /// # async fn f(mut client: grammers_client::ClientHandle) -> Result<(), Box<dyn std::error::Error>> {
    /// println!("Displaying full user information of the logged-in user:");
    /// dbg!(client.get_me().await?);
    /// # Ok(())
    /// # }
    /// ```
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
    ///
    /// # Examples
    ///
    /// ```
    /// # async fn f(chat: grammers_tl_types::enums::InputPeer, mut client: grammers_client::ClientHandle) -> Result<(), Box<dyn std::error::Error>> {
    /// let mut participants = client.iter_participants(&chat);
    ///
    /// while let Some(participant) = participants.next().await? {
    ///     println!("{} has role {:?}", participant.user.first_name.unwrap(), participant.role);
    /// }
    /// # Ok(())
    /// # }
    /// ```
    pub fn iter_participants(&self, chat: &tl::enums::InputPeer) -> ParticipantIter {
        ParticipantIter::new(self, chat)
    }

    /// Kicks the participant from the chat.
    ///
    /// This will fail if you do not have sufficient permissions to perform said operation.
    ///
    /// The kicked user will be able to join after being kicked (they are not permanently banned).
    ///
    /// Kicking someone who was not in the chat prior to running this method will be able to join
    /// after as well (effectively unbanning them).
    ///
    /// When used to kick users from "user" chat, nothing will be done.
    ///
    /// # Examples
    ///
    /// ```
    /// # async fn f(chat: grammers_tl_types::enums::InputPeer, user: grammers_tl_types::enums::InputUser, mut client: grammers_client::ClientHandle) -> Result<(), Box<dyn std::error::Error>> {
    /// match client.kick_participant(&chat, &user).await {
    ///     Ok(_) => println!("user is no more >:D"),
    ///     Err(_) => println!("Kick failed! Are you sure you're admin?"),
    /// };
    /// # Ok(())
    /// # }
    /// ```
    pub async fn kick_participant(
        &mut self,
        chat: &tl::enums::InputPeer,
        user: &tl::enums::InputUser,
    ) -> Result<(), InvocationError> {
        if let Some(channel) = chat.to_input_channel() {
            use tl::enums::InputUser::*;

            match user {
                Empty => Ok(()),
                UserSelf => self
                    .invoke(&tl::functions::channels::LeaveChannel { channel })
                    .await
                    .map(drop),
                User(_) | FromMessage(_) => {
                    // This will fail if the user represents ourself, but either verifying
                    // beforehand that the user is in fact ourselves or checking it after
                    // an error occurs is not really worth it.
                    self.set_banned_rights(&channel, user)
                        .view_messages(false)
                        .duration(Duration::from_secs(KICK_BAN_DURATION as u64))
                        .await?;

                    self.set_banned_rights(&channel, user).await
                }
            }
        } else if let Some(chat_id) = chat.to_chat_id() {
            self.invoke(&tl::functions::messages::DeleteChatUser {
                chat_id,
                user_id: user.clone(),
            })
            .await
            .map(drop)
        } else {
            Ok(())
        }
    }

    /// Set the banned rights for a specific user.
    ///
    /// Returns a new [`BannedRightsBuilder`] instance. Check out the documentation for that type
    /// to learn more about what restrictions can be applied.
    ///
    /// Nothing is done until the returned instance is awaited, at which point it might result in
    /// error if you do not have sufficient permissions to ban the user in the input chat.
    ///
    /// By default, the user has all rights, and you need to revoke those you want to take away
    /// from the user by setting the permissions to `false`. This means that not taking away any
    /// permissions will effectively unban someone, granting them all default user permissions.
    ///
    /// By default, the ban is applied forever, but this can be changed to a shorter duration.
    ///
    /// The default group rights are respected, despite individual restrictions.
    ///
    /// # Example
    ///
    /// ```
    /// # async fn f(chat: grammers_tl_types::enums::InputChannel, user: grammers_tl_types::enums::InputUser, mut client: grammers_client::ClientHandle) -> Result<(), Box<dyn std::error::Error>> {
    /// // This user keeps spamming pepe stickers, take the sticker permission away from them
    /// let res = client
    ///     .set_banned_rights(&chat, &user)
    ///     .send_stickers(false)
    ///     .await;
    ///
    /// match res {
    ///     Ok(_) => println!("No more sticker spam!"),
    ///     Err(_) => println!("Ban failed! Are you sure you're admin?"),
    /// };
    /// # Ok(())
    /// # }
    /// ```
    pub fn set_banned_rights(
        &mut self,
        channel: &tl::enums::InputChannel,
        user: &tl::enums::InputUser,
    ) -> BannedRightsBuilder {
        BannedRightsBuilder::new(self.clone(), channel.clone(), user.clone())
    }

    /// Set the administrator rights for a specific user.
    ///
    /// Returns a new [`AdminRightsBuilder`] instance. Check out the documentation for that
    /// type to learn more about what rights can be given to administrators.
    ///
    /// Nothing is done until the returned instance is awaited, at which point it might result in
    /// error if you do not have sufficient permissions to grant those rights to the other user.
    ///
    /// By default, no permissions are granted, and you need to specify those you want to grant by
    /// setting the permissions to `true`. This means that not granting any permission will turn
    /// the user into a normal user again, and they will no longer be an administrator.
    ///
    /// The change is applied forever and there is no way to set a specific duration. If the user
    /// should only be an administrator for a set period of time, the administrator permissions
    /// must be manually revoked at a later point in time.
    ///
    /// # Example
    ///
    /// ```
    /// # async fn f(chat: grammers_tl_types::enums::InputChannel, user: grammers_tl_types::enums::InputUser, mut client: grammers_client::ClientHandle) -> Result<(), Box<dyn std::error::Error>> {
    /// // Let the user pin messages and ban other people
    /// let res = client.set_admin_rights(&chat, &user)
    ///     .load_current()
    ///     .await?
    ///     .pin_messages(true)
    ///     .ban_users(true)
    ///     .await?;
    /// # Ok(())
    /// # }
    /// ```
    pub fn set_admin_rights(
        &mut self,
        channel: &tl::enums::InputChannel,
        user: &tl::enums::InputUser,
    ) -> AdminRightsBuilder {
        AdminRightsBuilder::new(self.clone(), channel.clone(), user.clone())
    }

    /// Iterate over the history of profile photos for the given user or chat.
    ///
    /// Note that the current photo might not be present in the history, and to avoid doing more
    /// work when it's generally not needed (the photo history tends to be complete but in some
    /// cases it might not be), it's up to you to fetch this photo from the full channel.
    ///
    /// Note that you cannot use these photos to send them as messages directly. They must be
    /// downloaded first, then uploaded, and finally sent.
    ///
    /// # Examples
    ///
    /// ```
    /// # async fn f(chat: grammers_tl_types::enums::InputPeer, mut client: grammers_client::ClientHandle) -> Result<(), Box<dyn std::error::Error>> {
    /// let mut photos = client.iter_profile_photos(&chat);
    ///
    /// while let Some(photo) = photos.next().await? {
    ///     println!("Did you know chat has a photo with ID {}?", photo.id);
    /// }
    /// # Ok(())
    /// # }
    /// ```
    pub fn iter_profile_photos(&self, chat: &tl::enums::InputPeer) -> ProfilePhotoIter {
        ProfilePhotoIter::new(self, chat)
    }
}
