use crate::ClientHandle;
use futures::FutureExt;
use grammers_mtsender::InvocationError;
use grammers_tl_types as tl;
use std::{
    future::Future,
    mem::drop,
    pin::Pin,
    task::{Context, Poll},
    time::{Duration, SystemTime, UNIX_EPOCH},
};

type FutOutput = Result<(), InvocationError>;
type FutStore = Pin<Box<dyn Future<Output = FutOutput> + Send>>;

/// Builder for Editing the Admin Rights of a User in a Channel or SuperGroup
///
/// See [`ClientHandle::edit_admin_rights`] for an example
pub struct EditAdminRightsBuilder {
    client: ClientHandle,
    channel: tl::enums::InputChannel,
    user: tl::enums::InputUser,
    rights: tl::types::ChatAdminRights,
    rank: String,
    fut: Option<FutStore>,
}

impl Future for EditAdminRightsBuilder {
    type Output = FutOutput;
    fn poll(mut self: Pin<&mut Self>, cx: &mut Context) -> Poll<FutOutput> {
        if self.fut.is_none() {
            let call = tl::functions::channels::EditAdmin {
                channel: self.channel.clone(),
                user_id: self.user.clone(),
                admin_rights: tl::enums::ChatAdminRights::Rights(self.rights.clone()),
                rank: self.rank.clone(),
            };
            let mut c = self.client.clone();
            self.fut = Some(Box::pin(async move {
                c.invoke(&call).map(|f| f.map(drop)).await
            }));
        }
        Future::poll(self.fut.as_mut().unwrap().as_mut(), cx)
    }
}

impl EditAdminRightsBuilder {
    pub(crate) fn new(
        client: ClientHandle,
        channel: tl::enums::InputChannel,
        user: tl::enums::InputUser,
    ) -> Self {
        Self {
            client,
            channel,
            user,
            rank: "".into(),
            rights: tl::types::ChatAdminRights {
                anonymous: false,
                change_info: false,
                post_messages: false,
                edit_messages: false,
                delete_messages: false,
                ban_users: false,
                invite_users: false,
                pin_messages: false,
                add_admins: false,
            },
            fut: None,
        }
    }

    /// Load current rights of the user
    pub async fn load_current(&mut self) -> Result<&mut Self, InvocationError> {
        let tl::enums::channels::ChannelParticipant::Participant(user) = self
            .client
            .invoke(&tl::functions::channels::GetParticipant {
                channel: self.channel.clone(),
                user_id: self.user.clone(),
            })
            .await?;
        match user.participant {
            tl::enums::ChannelParticipant::Creator(c) => {
                self.rights.change_info = true;
                self.rights.post_messages = true;
                self.rights.edit_messages = true;
                self.rights.delete_messages = true;
                self.rights.ban_users = true;
                self.rights.invite_users = true;
                self.rights.pin_messages = true;
                self.rights.add_admins = true;
                self.rank = c.rank.unwrap_or("".to_string())
            }
            tl::enums::ChannelParticipant::Admin(admin) => {
                let tl::enums::ChatAdminRights::Rights(rights) = admin.admin_rights;
                self.rights = rights;
                self.rank = admin.rank.unwrap_or("".to_string())
            }
            _ => (),
        }

        Ok(self)
    }

    /// Allow admin to be anonymous
    pub fn anonymous(&mut self, val: bool) -> &mut Self {
        self.rights.post_messages = val;
        self
    }

    /// Allow admin to post messages (Channel specific)
    pub fn post_messages(&mut self, val: bool) -> &mut Self {
        self.rights.post_messages = val;
        self
    }

    /// Allow admin to edit messages (Channel specific)
    pub fn edit_messages(&mut self, val: bool) -> &mut Self {
        self.rights.edit_messages = val;
        self
    }

    /// Allow admin to delete messages of other users
    pub fn delete_messages(&mut self, val: bool) -> &mut Self {
        self.rights.delete_messages = val;
        self
    }

    pub fn ban_users(&mut self, val: bool) -> &mut Self {
        self.rights.ban_users = val;
        self
    }

    pub fn invite_users(&mut self, val: bool) -> &mut Self {
        self.rights.invite_users = val;
        self
    }

    pub fn pin_messages(&mut self, val: bool) -> &mut Self {
        self.rights.pin_messages = val;
        self
    }

    /// Allow admin to add other admins
    pub fn add_admins(&mut self, val: bool) -> &mut Self {
        self.rights.add_admins = val;
        self
    }

    /// Custom admin badge
    pub fn rank<S: Into<String>>(&mut self, val: S) -> &mut Self {
        self.rank = val.into();
        self
    }
}

/// Builder for Editing the Banned Rights of a User in a Channel or SuperGroup
///
/// See [`ClientHandle::edit_banned_rights`] for an example
pub struct EditBannedRightsBuilder {
    client: ClientHandle,
    channel: tl::enums::InputChannel,
    user: tl::enums::InputUser,
    rights: tl::types::ChatBannedRights,
    fut: Option<FutStore>,
}

impl Future for EditBannedRightsBuilder {
    type Output = FutOutput;
    fn poll(mut self: Pin<&mut Self>, cx: &mut Context) -> Poll<FutOutput> {
        if self.fut.is_none() {
            let call = tl::functions::channels::EditBanned {
                channel: self.channel.clone(),
                user_id: self.user.clone(),
                banned_rights: tl::enums::ChatBannedRights::Rights(self.rights.clone()),
            };
            let mut c = self.client.clone();
            self.fut = Some(Box::pin(async move {
                c.invoke(&call).map(|f| f.map(drop)).await
            }));
        }
        Future::poll(self.fut.as_mut().unwrap().as_mut(), cx)
    }
}

impl EditBannedRightsBuilder {
    pub(crate) fn new(
        client: ClientHandle,
        channel: tl::enums::InputChannel,
        user: tl::enums::InputUser,
    ) -> Self {
        Self {
            client,
            channel,
            user,
            rights: tl::types::ChatBannedRights {
                view_messages: false,
                send_messages: false,
                send_media: false,
                send_stickers: false,
                send_gifs: false,
                send_games: false,
                send_inline: false,
                embed_links: false,
                send_polls: false,
                change_info: false,
                invite_users: false,
                pin_messages: false,
                until_date: 0,
            },
            fut: None,
        }
    }

    /// Load the default banned rights of current user in the channel
    pub async fn load_current(&mut self) -> Result<&mut Self, InvocationError> {
        let tl::enums::channels::ChannelParticipant::Participant(user) = self
            .client
            .invoke(&tl::functions::channels::GetParticipant {
                channel: self.channel.clone(),
                user_id: self.user.clone(),
            })
            .await?;
        match user.participant {
            tl::enums::ChannelParticipant::Banned(u) => {
                let tl::enums::ChatBannedRights::Rights(rights) = u.banned_rights;
                self.rights = rights;
            }
            _ => (),
        }

        Ok(self)
    }

    /// Allow user to view messages (aka ban)
    pub fn view_messages(&mut self, val: bool) -> &mut Self {
        // in tl::types::ChatBannedRights, true means to disable a right
        self.rights.view_messages = !val;
        self
    }

    pub fn send_messages(&mut self, val: bool) -> &mut Self {
        self.rights.send_messages = !val;
        self
    }

    pub fn send_media(&mut self, val: bool) -> &mut Self {
        self.rights.send_media = !val;
        self
    }

    pub fn send_stickers(&mut self, val: bool) -> &mut Self {
        self.rights.send_stickers = !val;
        self
    }

    pub fn send_gifs(&mut self, val: bool) -> &mut Self {
        self.rights.send_gifs = !val;
        self
    }

    pub fn send_games(&mut self, val: bool) -> &mut Self {
        self.rights.send_games = !val;
        self
    }

    /// Allow user to use inline bots
    pub fn send_inline(&mut self, val: bool) -> &mut Self {
        self.rights.send_inline = !val;
        self
    }

    /// Allow user to embed links in message
    pub fn embed_links(&mut self, val: bool) -> &mut Self {
        self.rights.embed_links = !val;
        self
    }

    /// Allow user to send polls
    pub fn send_polls(&mut self, val: bool) -> &mut Self {
        self.rights.send_polls = !val;
        self
    }

    /// Allow user to change group description
    pub fn change_info(&mut self, val: bool) -> &mut Self {
        self.rights.change_info = !val;
        self
    }

    pub fn invite_users(&mut self, val: bool) -> &mut Self {
        self.rights.invite_users = !val;
        self
    }

    pub fn pin_messages(&mut self, val: bool) -> &mut Self {
        self.rights.pin_messages = !val;
        self
    }

    /// Ban user till given epoch time
    /// WARN: this takes absolute time (i.e current time is not added)
    /// default: 0 (permanent)
    pub fn until_date(&mut self, val: i32) -> &mut Self {
        self.rights.until_date = val;
        self
    }

    /// Ban user for given time
    /// current time is added
    pub fn duration(&mut self, val: Duration) -> &mut Self {
        // TODO this should account for the server time instead (via sender's offset)
        self.rights.until_date = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time is before epoch")
            .as_secs() as i32
            + val.as_secs() as i32;

        self
    }
}
