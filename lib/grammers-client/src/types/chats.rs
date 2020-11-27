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
/// See [`ClientHandle::edit_admin_rights`] for an example
pub struct EditAdminRightsBuilder {
    client: ClientHandle,
    channel: tl::enums::InputChannel,
    user: tl::enums::InputUser,
    rights: tl::types::ChatAdminRights,
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
                admin_rights: tl::enums::ChatAdminRights::Rights(self.rights.clone()),                
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
    pub(crate) fn new(
        client: ClientHandle,
        channel: tl::enums::InputChannel,
        user: tl::enums::InputUser
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
                self.rights.change_info = true;
                self.rights.post_messages = true;
                self.rights.edit_messages = true;
                self.rights.delete_messages = true;
                self.rights.ban_users = true;
                self.rights.invite_users = true;
                self.rights.pin_messages = true;
                self.rights.add_admins = true;
                self.rank = c.rank.unwrap_or("".to_string())
            },
            tl::enums::ChannelParticipant::Admin(admin) => {
                let tl::enums::ChatAdminRights::Rights(rights) = admin.admin_rights;
                self.rights.change_info = rights.change_info;
                self.rights.post_messages = rights.post_messages;
                self.rights.edit_messages = rights.edit_messages;
                self.rights.delete_messages = rights.delete_messages;
                self.rights.ban_users = rights.ban_users;
                self.rights.invite_users = rights.invite_users;
                self.rights.pin_messages = rights.pin_messages;
                self.rights.add_admins = rights.add_admins;
                self.rank = admin.rank.unwrap_or("".to_string())
            },
            _ => ()
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
