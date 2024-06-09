// Copyright 2020 - developers of the `grammers` project.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// https://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or https://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

//! Methods related to users, groups and channels.

use super::Client;
use crate::types::{
    chats::AdminRightsBuilderInner, chats::BannedRightsBuilderInner, AdminRightsBuilder,
    BannedRightsBuilder, Chat, ChatMap, IterBuffer, Message, Participant, Photo, User,
};
use grammers_mtsender::RpcError;
pub use grammers_mtsender::{AuthorizationError, InvocationError};
use grammers_session::{PackedChat, PackedType};
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
        chat_id: i64,
        buffer: VecDeque<Participant>,
        total: Option<usize>,
    },
    Channel(IterBuffer<tl::functions::channels::GetParticipants, Participant>),
}

impl ParticipantIter {
    fn new(client: &Client, chat: PackedChat) -> Self {
        if let Some(channel) = chat.try_to_input_channel() {
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
        } else if let Some(chat_id) = chat.try_to_chat_id() {
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

                {
                    let mut state = client.0.state.write().unwrap();
                    // Telegram can return peers without hash (e.g. Users with 'min: true')
                    let _ = state.chat_hashes.extend(&full.users, &full.chats);
                }

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
                let (count, participants, chats, users) =
                    match iter.client.invoke(&iter.request).await? {
                        Participants(p) => (p.count, p.participants, p.chats, p.users),
                        NotModified => {
                            panic!("API returned Dialogs::NotModified even though hash = 0")
                        }
                    };

                {
                    let mut state = iter.client.0.state.write().unwrap();
                    // Telegram can return peers without hash (e.g. Users with 'min: true')
                    let _ = state.chat_hashes.extend(&users, &chats);
                }

                // Telegram can return less participants than asked for but the count being higher
                // (for example, count=4825, participants=199, users=200). The missing participant
                // was an admin bot account, not sure why it's not included.
                //
                // In any case we pick whichever size is highest to avoid weird cases like this.
                iter.last_chunk =
                    usize::max(participants.len(), users.len()) < iter.request.limit as usize;
                iter.request.offset += participants.len() as i32;

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

    /// apply a filter on fetched participants, note that this filter will apply only on large `Channel` and not small groups
    pub fn filter(mut self, filter: tl::enums::ChannelParticipantsFilter) -> Self {
        match self {
            ParticipantIter::Channel(ref mut c) => {
                c.request.filter = filter;
                self
            }
            _ => self,
        }
    }
}

pub enum ProfilePhotoIter {
    User(IterBuffer<tl::functions::photos::GetUserPhotos, Photo>),
    Chat(IterBuffer<tl::functions::messages::Search, Message>),
}

impl ProfilePhotoIter {
    fn new(client: &Client, chat: PackedChat) -> Self {
        if let Some(user_id) = chat.try_to_input_user() {
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
    /// # async fn f(client: grammers_client::Client) -> Result<(), Box<dyn std::error::Error>> {
    /// if let Some(chat) = client.resolve_username("username").await? {
    ///     println!("Found chat!: {:?}", chat.name());
    /// }
    /// # Ok(())
    /// # }
    /// ```
    pub async fn resolve_username(&self, username: &str) -> Result<Option<Chat>, InvocationError> {
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

        {
            let mut state = self.0.state.write().unwrap();
            // Telegram can return peers without hash (e.g. Users with 'min: true')
            let _ = state.chat_hashes.extend(&users, &chats);
        }

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
                .map(Chat::from_raw)
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
    /// # async fn f(client: grammers_client::Client) -> Result<(), Box<dyn std::error::Error>> {
    /// println!("Displaying full user information of the logged-in user:");
    /// dbg!(client.get_me().await?);
    /// # Ok(())
    /// # }
    /// ```
    pub async fn get_me(&self) -> Result<User, InvocationError> {
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
    /// # async fn f(chat: grammers_client::types::Chat, client: grammers_client::Client) -> Result<(), Box<dyn std::error::Error>> {
    /// let mut participants = client.iter_participants(&chat);
    ///
    /// while let Some(participant) = participants.next().await? {
    ///     println!("{} has role {:?}", participant.user.first_name(), participant.role);
    /// }
    /// # Ok(())
    /// # }
    /// ```
    pub fn iter_participants<C: Into<PackedChat>>(&self, chat: C) -> ParticipantIter {
        ParticipantIter::new(self, chat.into())
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
    /// # async fn f(chat: grammers_client::types::Chat, user: grammers_client::types::User, client: grammers_client::Client) -> Result<(), Box<dyn std::error::Error>> {
    /// match client.kick_participant(&chat, &user).await {
    ///     Ok(_) => println!("user is no more >:D"),
    ///     Err(_) => println!("Kick failed! Are you sure you're admin?"),
    /// };
    /// # Ok(())
    /// # }
    /// ```
    pub async fn kick_participant<C: Into<PackedChat>, U: Into<PackedChat>>(
        &self,
        chat: C,
        user: U,
    ) -> Result<(), InvocationError> {
        let chat = chat.into();
        let user = user.into();
        if let Some(channel) = chat.try_to_input_channel() {
            // TODO should PackedChat also know about is user self?
            let self_id = { self.0.state.read().unwrap().chat_hashes.self_id() };
            if user.id == self_id {
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
        } else if let Some(chat_id) = chat.try_to_chat_id() {
            self.invoke(&tl::functions::messages::DeleteChatUser {
                chat_id,
                user_id: user.to_input_user_lossy(),
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
    /// # async fn f(chat: grammers_client::types::Chat, user: grammers_client::types::User, client: grammers_client::Client) -> Result<(), Box<dyn std::error::Error>> {
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
    pub fn set_banned_rights<C: Into<PackedChat>, U: Into<PackedChat>>(
        &self,
        channel: C,
        user: U,
    ) -> BannedRightsBuilder<impl Future<Output = Result<(), InvocationError>>> {
        BannedRightsBuilder::new(
            self.clone(),
            channel.into(),
            user.into(),
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
    /// # async fn f(chat: grammers_client::types::Chat, user: grammers_client::types::User, client: grammers_client::Client) -> Result<(), Box<dyn std::error::Error>> {
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
    pub fn set_admin_rights<C: Into<PackedChat>, U: Into<PackedChat>>(
        &self,
        channel: C,
        user: U,
    ) -> AdminRightsBuilder<impl Future<Output = Result<(), InvocationError>>> {
        AdminRightsBuilder::new(
            self.clone(),
            channel.into(),
            user.into(),
            AdminRightsBuilderInner::invoke,
        )
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
    /// # async fn f(chat: grammers_client::types::Chat, client: grammers_client::Client) -> Result<(), Box<dyn std::error::Error>> {
    /// let mut photos = client.iter_profile_photos(&chat);
    ///
    /// while let Some(photo) = photos.next().await? {
    ///     println!("Did you know chat has a photo with ID {}?", photo.id());
    /// }
    /// # Ok(())
    /// # }
    /// ```
    pub fn iter_profile_photos<C: Into<PackedChat>>(&self, chat: C) -> ProfilePhotoIter {
        ProfilePhotoIter::new(self, chat.into())
    }

    /// Convert a [`PackedChat`] back into a [`Chat`].
    ///
    /// # Example
    ///
    /// ```
    /// # async fn f(packed_chat: grammers_client::types::chat::PackedChat, client: grammers_client::Client) -> Result<(), Box<dyn std::error::Error>> {
    /// let chat = client.unpack_chat(packed_chat).await?;
    ///
    /// println!("Found chat: {}", chat.name());
    /// # Ok(())
    /// # }
    /// ```
    pub async fn unpack_chat(&self, packed_chat: PackedChat) -> Result<Chat, InvocationError> {
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
                Chat::from_raw(res.pop().unwrap())
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
                Chat::from_raw(res.pop().unwrap())
            }
        })
    }

    /// Get permissions of participant `user` from chat `chat`.
    ///
    /// # Panics
    /// Panics if chat isn't channel or chat, and if user isn't user
    pub async fn get_permissions<C: Into<PackedChat>, U: Into<PackedChat>>(
        &self,
        chat: C,
        user: U,
    ) -> Result<ParticipantPermissions, InvocationError> {
        let chat = chat.into();
        let user = user.into();
        if user.try_to_input_user().is_none() {
            panic!("User parameter not user!");
        }

        // Get by chat
        if let Some(chat_id) = chat.try_to_chat_id() {
            // Get user id
            let user = user.try_to_input_user().unwrap();
            let user_id = match user {
                tl::enums::InputUser::User(user) => user.user_id,
                tl::enums::InputUser::FromMessage(user) => user.user_id,
                tl::enums::InputUser::UserSelf => {
                    let me = self.get_me().await?;
                    me.id()
                }
                tl::enums::InputUser::Empty => unreachable!(),
            };

            // Get chat and find user
            let chat = self
                .invoke(&tl::functions::messages::GetFullChat { chat_id })
                .await?;
            let tl::enums::messages::ChatFull::Full(chat) = chat;
            if let tl::enums::ChatFull::Full(chat) = chat.full_chat {
                if let tl::enums::ChatParticipants::Participants(participants) = chat.participants {
                    for participant in participants.participants {
                        if participant.user_id() == user_id {
                            return Ok(ParticipantPermissions::Chat(participant));
                        }
                    }
                }
            }
            return Err(InvocationError::Rpc(RpcError {
                code: 400,
                name: "USER_NOT_PARTICIPANT".to_string(),
                value: None,
                caused_by: None,
            }));
        }

        // Get by channel
        let participant = self
            .invoke(&tl::functions::channels::GetParticipant {
                channel: chat.try_to_input_channel().unwrap(),
                participant: user.to_input_peer(),
            })
            .await?;
        let tl::enums::channels::ChannelParticipant::Participant(participant) = participant;
        let permissions = ParticipantPermissions::Channel(participant.participant);
        Ok(permissions)
    }

    #[cfg(feature = "parse_invite_link")]
    fn parse_invite_link(invite_link: &str) -> Option<String> {
        let url_parse_result = url::Url::parse(invite_link);
        if url_parse_result.is_err() {
            return None;
        }

        let url_parse = url_parse_result.unwrap();
        let scheme = url_parse.scheme();
        let path = url_parse.path();
        if url_parse.host_str().is_none() || !vec!["https", "http"].contains(&scheme) {
            return None;
        }
        let host = url_parse.host_str().unwrap();
        let hosts = [
            "t.me",
            "telegram.me",
            "telegram.dog",
            "tg.dev",
            "telegram.me",
            "telesco.pe",
        ];

        if !hosts.contains(&host) {
            return None;
        }
        let paths = path.split("/").collect::<Vec<&str>>();

        if paths.len() == 1 {
            if paths[0].starts_with("+") {
                return Some(paths[0].replace("+", ""));
            }
            return None;
        }

        if paths.len() > 1 {
            if paths[0].starts_with("joinchat") {
                return Some(paths[1].to_string());
            }
            if paths[0].starts_with("+") {
                return Some(paths[0].replace("+", ""));
            }
            return None;
        }

        None
    }

    /// Accept an invite link to join the corresponding private chat.
    ///
    /// If the chat is public (has a public username), [`Client::join_chat`](Client::join_chat) should be used instead.
    #[cfg(feature = "parse_invite_link")]
    pub async fn accept_invite_link(
        &self,
        invite_link: &str,
    ) -> Result<tl::enums::Updates, InvocationError> {
        match Self::parse_invite_link(invite_link) {
            Some(hash) => {
                self.invoke(&tl::functions::messages::ImportChatInvite { hash })
                    .await
            }
            None => Err(InvocationError::Rpc(RpcError {
                code: 400,
                name: "INVITE_HASH_INVALID".to_string(),
                value: None,
                caused_by: None,
            })),
        }
    }

    #[cfg_attr(
        not(feature = "parse_invite_link"),
        allow(rustdoc::broken_intra_doc_links)
    )]
    /// Join a public group or channel.
    ///
    /// A channel is public if it has a username.
    /// To join private chats, [`Client::accept_invite_link`](Client::accept_invite_link) should be used instead.
    pub async fn join_chat<C: Into<PackedChat>>(
        &self,
        chat: C,
    ) -> Result<Option<Chat>, InvocationError> {
        use tl::enums::Updates;

        let chat = chat.into();
        let update_chat = match self
            .invoke(&tl::functions::channels::JoinChannel {
                channel: chat.try_to_input_channel().unwrap(),
            })
            .await?
        {
            Updates::Combined(updates) => Some(
                updates
                    .chats
                    .into_iter()
                    .filter(|x| x.id() == chat.id)
                    .collect::<Vec<tl::enums::Chat>>(),
            ),
            Updates::Updates(updates) => Some(
                updates
                    .chats
                    .into_iter()
                    .filter(|x| x.id() == chat.id)
                    .collect::<Vec<tl::enums::Chat>>(),
            ),
            _ => None,
        };

        match update_chat {
            Some(chats) if !chats.is_empty() => Ok(Some(Chat::from_raw(chats[0].clone()))),
            Some(chats) if chats.is_empty() => Ok(None),
            None => Ok(None),
            Some(_) => Ok(None),
        }
    }
}

#[derive(Debug, Clone)]
pub enum ParticipantPermissions {
    Channel(tl::enums::ChannelParticipant),
    Chat(tl::enums::ChatParticipant),
}

impl ParticipantPermissions {
    /// Whether the user is the creator of the chat or not.
    pub fn is_creator(&self) -> bool {
        matches!(
            self,
            Self::Channel(tl::enums::ChannelParticipant::Creator(_))
                | Self::Chat(tl::enums::ChatParticipant::Creator(_))
        )
    }

    /// Whether the user is an administrator of the chat or not. The creator also counts as begin an administrator, since they have all permissions.
    pub fn is_admin(&self) -> bool {
        self.is_creator()
            || matches!(
                self,
                Self::Channel(tl::enums::ChannelParticipant::Admin(_))
                    | Self::Chat(tl::enums::ChatParticipant::Admin(_))
            )
    }

    /// Whether the user is banned in the chat.
    pub fn is_banned(&self) -> bool {
        matches!(
            self,
            Self::Channel(tl::enums::ChannelParticipant::Banned(_))
        )
    }

    /// Whether the user left the chat.
    pub fn has_left(&self) -> bool {
        matches!(self, Self::Channel(tl::enums::ChannelParticipant::Left(_)))
    }

    /// Whether the user is a normal user of the chat (not administrator, but not banned either, and has no restrictions applied).
    pub fn has_default_permissions(&self) -> bool {
        matches!(
            self,
            Self::Channel(tl::enums::ChannelParticipant::Participant(_))
                | Self::Channel(tl::enums::ChannelParticipant::ParticipantSelf(_))
                | Self::Chat(tl::enums::ChatParticipant::Participant(_))
        )
    }

    /// Whether the administrator can add new administrators with the same or less permissions than them.
    pub fn can_add_admins(&self) -> bool {
        if !self.is_admin() {
            return false;
        }
        match self {
            Self::Channel(tl::enums::ChannelParticipant::Admin(participant)) => {
                let tl::enums::ChatAdminRights::Rights(rights) = &participant.admin_rights;
                rights.add_admins
            }
            Self::Channel(tl::enums::ChannelParticipant::Creator(_)) => true,
            Self::Chat(_) => self.is_creator(),
            _ => false,
        }
    }
}
