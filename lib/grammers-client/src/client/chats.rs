// Copyright 2020 - developers of the `grammers` project.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// https://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or https://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

//! Methods related to users, groups and channels.

use super::Client;
use crate::types::chat::PackedType;
use crate::types::{
    chat::PackedChat, chats::AdminRightsBuilderInner, chats::BannedRightsBuilderInner,
    AdminRightsBuilder, BannedRightsBuilder, Chat, ChatMap, IterBuffer, Message, Participant,
    Photo, User,
};
pub use grammers_mtsender::{AuthorizationError, InvocationError};
use grammers_tl_types as tl;
use std::collections::VecDeque;
use std::future::Future;
use std::sync::Arc;
use std::time::Duration;

const MAX_PARTICIPANT_LIMIT: usize = 200;
const MAX_PHOTO_LIMIT: usize = 100;
const KICK_BAN_DURATION: i32 = 60; // in seconds, in case the second request fails

pub enum ParticipantIter {
    Empty,
    Chat {
        client: Client,
        chat_id: i32,
        buffer: VecDeque<Participant>,
        total: Option<usize>,
    },
    Channel(IterBuffer<tl::functions::channels::GetParticipants, Participant>),
}

impl ParticipantIter {
    fn new(client: &Client, chat: &Chat) -> Self {
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

                // Don't actually care for the chats, just the users.
                let mut chats = ChatMap::new(full.users, Vec::new());
                let chats = Arc::get_mut(&mut chats).unwrap();

                buffer.extend(
                    participants
                        .into_iter()
                        .map(|p| Participant::from_raw_chat(chats, p)),
                );

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

                // Don't actually care for the chats, just the users.
                let mut chats = ChatMap::new(users, Vec::new());
                let chats = Arc::get_mut(&mut chats).unwrap();

                iter.buffer.extend(
                    participants
                        .into_iter()
                        .map(|p| Participant::from_raw_channel(chats, p)),
                );

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
    User(IterBuffer<tl::functions::photos::GetUserPhotos, Photo>),
    Chat(IterBuffer<tl::functions::messages::Search, Message>),
}

impl ProfilePhotoIter {
    fn new(client: &Client, chat: &Chat) -> Self {
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

                let client = &iter.client;
                iter.buffer.extend(
                    photos
                        .into_iter()
                        .map(|x| Photo::from_raw(x, client.clone())),
                );

                Ok(total)
            }
            Self::Chat(_) => panic!("fill_buffer should not be called for Chat variant"),
        }
    }

    /// Return the next photo from the internal buffer, filling the buffer previously if it's
    /// empty.
    ///
    /// Returns `None` if the `limit` is reached or there are no photos left.
    pub async fn next(&mut self) -> Result<Option<Photo>, InvocationError> {
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
                    if let Some(tl::enums::MessageAction::ChatEditPhoto(
                        tl::types::MessageActionChatEditPhoto { photo },
                    )) = message.action
                    {
                        return Ok(Some(Photo::from_raw(photo, message.client.clone())));
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

/// Method implementations related to dealing with chats or other users.
impl Client {
    /// Resolves a username into the chat that owns it, if any.
    ///
    /// Note that this method is expensive to call, and can quickly cause long flood waits.
    ///
    /// # Examples
    ///
    /// ```
    /// # async fn f(mut client: grammers_client::Client) -> Result<(), Box<dyn std::error::Error>> {
    /// if let Some(chat) = client.resolve_username("username").await? {
    ///     println!("Found chat!: {:?}", chat.name());
    /// }
    /// # Ok(())
    /// # }
    /// ```
    pub async fn resolve_username(
        &mut self,
        username: &str,
    ) -> Result<Option<Chat>, InvocationError> {
        let tl::types::contacts::ResolvedPeer { peer, users, chats } = match self
            .invoke(&tl::functions::contacts::ResolveUsername {
                username: username.into(),
            })
            .await
        {
            Ok(tl::enums::contacts::ResolvedPeer::Peer(p)) => p,
            Err(err) if err.is("USERNAME_NOT_OCCUPIED") => return Ok(None),
            Err(err) => return Err(err),
        };

        Ok(match peer {
            tl::enums::Peer::User(tl::types::PeerUser { user_id }) => users
                .into_iter()
                .map(Chat::from_user)
                .find(|chat| chat.id() == user_id),
            tl::enums::Peer::Chat(tl::types::PeerChat { chat_id })
            | tl::enums::Peer::Channel(tl::types::PeerChannel {
                channel_id: chat_id,
            }) => chats
                .into_iter()
                .map(Chat::from_chat)
                .find(|chat| chat.id() == chat_id),
        })
    }

    /// Fetch full information about the currently logged-in user.
    ///
    /// Although this method is cheap to call, you might want to cache the results somewhere.
    ///
    /// # Examples
    ///
    /// ```
    /// # async fn f(mut client: grammers_client::Client) -> Result<(), Box<dyn std::error::Error>> {
    /// println!("Displaying full user information of the logged-in user:");
    /// dbg!(client.get_me().await?);
    /// # Ok(())
    /// # }
    /// ```
    pub async fn get_me(&mut self) -> Result<User, InvocationError> {
        let mut res = self
            .invoke(&tl::functions::users::GetUsers {
                id: vec![tl::enums::InputUser::UserSelf],
            })
            .await?;

        if res.len() != 1 {
            panic!("fetching only one user should exactly return one user");
        }

        Ok(User::from_raw(res.pop().unwrap()))
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
    /// # async fn f(chat: grammers_client::types::Chat, mut client: grammers_client::Client) -> Result<(), Box<dyn std::error::Error>> {
    /// let mut participants = client.iter_participants(&chat);
    ///
    /// while let Some(participant) = participants.next().await? {
    ///     println!("{} has role {:?}", participant.user.first_name(), participant.role);
    /// }
    /// # Ok(())
    /// # }
    /// ```
    pub fn iter_participants(&self, chat: &Chat) -> ParticipantIter {
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
    /// # async fn f(chat: grammers_client::types::Chat, user: grammers_client::types::User, mut client: grammers_client::Client) -> Result<(), Box<dyn std::error::Error>> {
    /// match client.kick_participant(&chat, &user).await {
    ///     Ok(_) => println!("user is no more >:D"),
    ///     Err(_) => println!("Kick failed! Are you sure you're admin?"),
    /// };
    /// # Ok(())
    /// # }
    /// ```
    pub async fn kick_participant(
        &mut self,
        chat: &Chat,
        user: &User,
    ) -> Result<(), InvocationError> {
        if let Some(channel) = chat.to_input_channel() {
            if user.is_self() {
                self.invoke(&tl::functions::channels::LeaveChannel { channel })
                    .await
                    .map(drop)
            } else {
                self.set_banned_rights(chat, user)
                    .view_messages(false)
                    .duration(Duration::from_secs(KICK_BAN_DURATION as u64))
                    .await?;

                self.set_banned_rights(chat, user).await
            }
        } else if let Some(chat_id) = chat.to_chat_id() {
            self.invoke(&tl::functions::messages::DeleteChatUser {
                chat_id,
                user_id: user.to_input(),
                revoke_history: false,
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
    /// Nothing is done until it is awaited, at which point it might result in
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
    /// # async fn f(chat: grammers_client::types::Chat, user: grammers_client::types::User, mut client: grammers_client::Client) -> Result<(), Box<dyn std::error::Error>> {
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
        channel: &Chat,
        user: &User,
    ) -> BannedRightsBuilder<impl Future<Output = Result<(), InvocationError>>> {
        BannedRightsBuilder::new(
            self.clone(),
            channel,
            user,
            BannedRightsBuilderInner::invoke,
        )
    }

    /// Set the administrator rights for a specific user.
    ///
    /// Returns a new [`AdminRightsBuilder`] instance. Check out the documentation for that
    /// type to learn more about what rights can be given to administrators.
    ///
    /// Nothing is done until it is awaited, at which point
    /// it might result in error if you do not have sufficient permissions to grant those rights
    /// to the other user.
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
    /// # async fn f(chat: grammers_client::types::Chat, user: grammers_client::types::User, mut client: grammers_client::Client) -> Result<(), Box<dyn std::error::Error>> {
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
        &self,
        channel: &Chat,
        user: &User,
    ) -> AdminRightsBuilder<impl Future<Output = Result<(), InvocationError>>> {
        AdminRightsBuilder::new(self.clone(), channel, user, AdminRightsBuilderInner::invoke)
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
    /// # async fn f(chat: grammers_client::types::Chat, mut client: grammers_client::Client) -> Result<(), Box<dyn std::error::Error>> {
    /// let mut photos = client.iter_profile_photos(&chat);
    ///
    /// while let Some(photo) = photos.next().await? {
    ///     println!("Did you know chat has a photo with ID {}?", photo.id());
    /// }
    /// # Ok(())
    /// # }
    /// ```
    pub fn iter_profile_photos(&self, chat: &Chat) -> ProfilePhotoIter {
        ProfilePhotoIter::new(self, chat)
    }

    /// Convert a [`PackedChat`] back into a [`Chat`]
    ///
    /// # Example
    ///
    /// ```
    /// # async fn f(packed_chat: grammers_client::types::chat::PackedChat, mut client: grammers_client::Client) -> Result<(), Box<dyn std::error::Error>> {
    /// let chat = client.unpack_chat(&packed_chat).await?;
    ///
    /// println!("Found chat: {}", chat.name());
    /// # Ok(())
    /// # }
    /// ```
    pub async fn unpack_chat(&mut self, packed_chat: &PackedChat) -> Result<Chat, InvocationError> {
        Ok(match packed_chat.ty {
            PackedType::User | PackedType::Bot => {
                let mut res = self
                    .invoke(&tl::functions::users::GetUsers {
                        id: vec![tl::enums::InputUser::User(tl::types::InputUser {
                            user_id: packed_chat.id,
                            access_hash: packed_chat.access_hash.unwrap(),
                        })],
                    })
                    .await?;
                if res.len() != 1 {
                    panic!("fetching only one user should exactly return one user");
                }
                Chat::from_user(res.pop().unwrap())
            }
            PackedType::Chat => {
                let mut res = match self
                    .invoke(&tl::functions::messages::GetChats {
                        id: vec![packed_chat.id],
                    })
                    .await?
                {
                    tl::enums::messages::Chats::Chats(chats) => chats.chats,
                    tl::enums::messages::Chats::Slice(chat_slice) => chat_slice.chats,
                };
                if res.len() != 1 {
                    panic!("fetching only one chat should exactly return one chat");
                }
                Chat::from_chat(res.pop().unwrap())
            }
            PackedType::Megagroup | PackedType::Broadcast | PackedType::Gigagroup => {
                let mut res = match self
                    .invoke(&tl::functions::channels::GetChannels {
                        id: vec![tl::enums::InputChannel::Channel(tl::types::InputChannel {
                            channel_id: packed_chat.id,
                            access_hash: packed_chat.access_hash.unwrap(),
                        })],
                    })
                    .await?
                {
                    tl::enums::messages::Chats::Chats(chats) => chats.chats,
                    tl::enums::messages::Chats::Slice(chat_slice) => chat_slice.chats,
                };
                if res.len() != 1 {
                    panic!("fetching only one chat should exactly return one chat");
                }
                Chat::from_chat(res.pop().unwrap())
            }
        })
    }
}
