use crate::{
    ClientHandle,
};
use futures::FutureExt;
use grammers_mtsender::InvocationError;
use grammers_tl_types as tl;
use std::{
    mem::drop,
    future::Future,
    task::{Context, Poll},
    pin::Pin
};


/// Builder for Editing the Admin Rights of a User in a Channel or SuperGroup
///
/// # Example
///
/// ```
/// # async fn f(chat: grammers_tl_types::enums::InputChannel, user: grammers_tl_types::enums::InputUser, mut client: grammers_client::ClientHandle) -> Result<(), Box<dyn std::error::Error>> {
/// let res = client.edit_admin_rights(&chat, &user).
///             load_current()
///             .await?
///             .pin_messages(true)
///             .invite_users(true)
///             .ban_users(true)
///             .await?;
/// # Ok(())
/// # }
/// ```
///
pub struct EditAdminRightsBuilder {
    client: ClientHandle,
    channel: tl::enums::InputChannel,
    user: tl::enums::InputUser,
    anonymous: bool,
    change_info: bool,
    post_messages: bool,
    edit_messages: bool,
    delete_messages: bool,
    ban_users: bool,
    invite_users: bool,
    pin_messages: bool,
    add_admins: bool,
    rank: String,
    fut: Option<Pin<Box<dyn Future<Output = Result<(), InvocationError>> + Send>>>
}

impl Future for EditAdminRightsBuilder {
    type Output = Result<(), InvocationError>;
    fn poll(mut self: Pin<&mut Self>, cx: &mut Context) -> Poll<Self::Output> {
        if self.fut.is_none() {
            let call = tl::functions::channels::EditAdmin {
                channel: self.channel.clone(),
                user_id: self.user.clone(),
                admin_rights: tl::enums::ChatAdminRights::Rights(
                    tl::types::ChatAdminRights {
                        anonymous: self.anonymous,
                        change_info: self.change_info,
                        post_messages: self.post_messages,
                        edit_messages: self.edit_messages,
                        delete_messages: self.delete_messages,
                        ban_users: self.ban_users,
                        invite_users: self.invite_users,
                        pin_messages: self.pin_messages,
                        add_admins: self.add_admins,
                    }
                ),
                rank: self.rank.clone()
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
    pub fn new(
        client: ClientHandle,
        channel: tl::enums::InputChannel,
        user: tl::enums::InputUser
    ) -> Self {
        Self {
            client,
            channel,
            user,
            anonymous: false,
            change_info: false,
            post_messages: false,
            edit_messages: false,
            delete_messages: false,
            ban_users: false,
            invite_users: false,
            pin_messages: false,
            add_admins: false,
            rank: "".into(),
            fut: None
        }
    }

    /// Load current rights of the user
    pub async fn load_current(&mut self) -> Result<&mut Self, InvocationError> {
        let tl::enums::channels::ChannelParticipant::Participant(user) = self.client.invoke(
            &tl::functions::channels::GetParticipant {
                channel: self.channel.clone(),
                user_id: self.user.clone()
            }
        ).await?;
        match user.participant {
            tl::enums::ChannelParticipant::Creator(c) => {
                self.change_info = true;
                self.post_messages = true;
                self.edit_messages = true;
                self.delete_messages = true;
                self.ban_users = true;
                self.invite_users = true;
                self.pin_messages = true;
                self.add_admins = true;
                self.rank = c.rank.unwrap_or("owner".to_string())
            },
            tl::enums::ChannelParticipant::Admin(admin) => {
                let tl::enums::ChatAdminRights::Rights(rights) = admin.admin_rights;
                self.change_info = rights.change_info;
                self.post_messages = rights.post_messages;
                self.edit_messages = rights.edit_messages;
                self.delete_messages = rights.delete_messages;
                self.ban_users = rights.ban_users;
                self.invite_users = rights.invite_users;
                self.pin_messages = rights.pin_messages;
                self.add_admins = rights.add_admins;
                self.rank = admin.rank.unwrap_or("admin".to_string())
            },
            _ => ()
        }

        Ok(self)
    }

    /// Allow admin to be anonymous
    pub fn anonymous(&mut self, val: bool) -> &mut Self {
        self.post_messages = val;
        self
    }

    /// Allow admin to post messages (Channel specific)
    pub fn post_messages(&mut self, val: bool) -> &mut Self {
        self.post_messages = val;
        self
    }

    /// Allow admin to edit messages (Channel specific)
    pub fn edit_messages(&mut self, val: bool) -> &mut Self {
        self.edit_messages = val;
        self
    }

    /// Allow admin to delete messages of other users
    pub fn delete_messages(&mut self, val: bool) -> &mut Self {
        self.delete_messages = val;
        self
    }

    pub fn ban_users(&mut self, val: bool) -> &mut Self {
        self.ban_users = val;
        self
    }

    pub fn invite_users(&mut self, val: bool) -> &mut Self {
        self.invite_users = val;
        self
    }

    pub fn pin_messages(&mut self, val: bool) -> &mut Self {
        self.pin_messages = val;
        self
    }

    /// Allow admin to add other admins
    pub fn add_admins(&mut self, val: bool) -> &mut Self {
        self.add_admins = val;
        self
    }

    /// Custom admin badge
    pub fn rank<S: Into<String>>(&mut self, val: S) -> &mut Self {
        self.rank = val.into();
        self
    }
}
