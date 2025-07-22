// Copyright 2020 - developers of the `grammers` project.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// https://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or https://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.
use grammers_session::{PackedChat, PackedType};
use grammers_tl_types as tl;
use std::fmt;

/// Platform Identifier.
#[non_exhaustive]
pub enum Platform {
    All,
    Android,
    Ios,
    WindowsPhone,
    Other(String),
}

/// Contains the reason why a certain user is restricted.
pub struct RestrictionReason {
    pub platforms: Vec<Platform>,
    pub reason: String,
    pub text: String,
}

impl RestrictionReason {
    pub fn from_raw(reason: &tl::enums::RestrictionReason) -> Self {
        let tl::enums::RestrictionReason::Reason(reason) = reason;
        Self {
            platforms: reason
                .platform
                .split('-')
                .map(|p| match p {
                    // Taken from https://core.telegram.org/constructor/restrictionReason
                    "all" => Platform::All,
                    "android" => Platform::Android,
                    "ios" => Platform::Ios,
                    "wp" => Platform::WindowsPhone,
                    o => Platform::Other(o.to_string()),
                })
                .collect(),
            reason: reason.reason.to_string(),
            text: reason.text.to_string(),
        }
    }
}

/// A user.
///
/// Users include your contacts, members of a group, bot accounts created by [@BotFather], or
/// anyone with a Telegram account.
///
/// A "normal" (non-bot) user may also behave like a "bot" without actually being one, for
/// example, when controlled with a program as opposed to being controlled by a human through
/// a Telegram application. These are commonly known as "userbots", and some people use them
/// to enhance their Telegram experience (for example, creating "commands" so that the program
/// automatically reacts to them, like translating messages).
///
/// [@BotFather]: https://t.me/BotFather
#[derive(Clone)]
pub struct User {
    pub raw: tl::enums::User,
}

impl fmt::Debug for User {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.raw.fmt(f)
    }
}

// TODO: photo
impl User {
    pub fn from_raw(user: tl::enums::User) -> Self {
        Self { raw: user }
    }

    pub(crate) fn empty_with_hash_and_bot(id: i64, access_hash: Option<i64>, bot: bool) -> Self {
        Self {
            raw: tl::enums::User::User(tl::types::User {
                is_self: false,
                contact: false,
                mutual_contact: false,
                deleted: false,
                bot,
                bot_chat_history: false,
                bot_nochats: false,
                verified: false,
                restricted: false,
                min: false, // not min because the input hash is not a min hash
                bot_inline_geo: false,
                support: false,
                scam: false,
                apply_min_photo: false,
                fake: false,
                bot_attach_menu: false,
                premium: false,
                attach_menu_enabled: false,
                bot_can_edit: false,
                close_friend: false,
                stories_hidden: false,
                stories_unavailable: true,
                contact_require_premium: false,
                bot_business: false,
                bot_has_main_app: false,
                id,
                access_hash,
                first_name: None,
                last_name: None,
                username: None,
                phone: None,
                photo: None,
                status: None,
                bot_info_version: None,
                restriction_reason: None,
                bot_inline_placeholder: None,
                lang_code: None,
                emoji_status: None,
                usernames: None,
                stories_max_id: None,
                color: None,
                profile_color: None,
                bot_active_users: None,
                bot_verification_icon: None,
                send_paid_messages_stars: None,
            }),
        }
    }

    pub(crate) fn user(&self) -> Option<&tl::types::User> {
        match &self.raw {
            tl::enums::User::User(u) => Some(u),
            tl::enums::User::Empty(_) => None,
        }
    }

    /// Return the user presence status (also known as "last seen").
    pub fn status(&self) -> &grammers_tl_types::enums::UserStatus {
        self.user()
            .and_then(|u| u.status.as_ref())
            .unwrap_or(&grammers_tl_types::enums::UserStatus::Empty)
    }

    /// Return the unique identifier for this user.
    pub fn id(&self) -> i64 {
        self.raw.id()
    }

    pub(crate) fn access_hash(&self) -> Option<i64> {
        self.user().and_then(|u| u.access_hash)
    }

    /// Pack this user into a smaller representation that can be loaded later.
    pub fn pack(&self) -> PackedChat {
        PackedChat {
            ty: if self.is_bot() {
                PackedType::Bot
            } else {
                PackedType::User
            },
            id: self.id(),
            access_hash: self.access_hash(),
        }
    }

    /// Return the first name of this user.
    ///
    /// The name will be `None` if the account was deleted. It may also be `None` if you received
    /// it previously.
    pub fn first_name(&self) -> Option<&str> {
        self.user().and_then(|u| u.first_name.as_deref())
    }

    /// Return the last name of this user, if any.
    pub fn last_name(&self) -> Option<&str> {
        self.user().and_then(|u| {
            u.last_name
                .as_deref()
                .and_then(|name| if name.is_empty() { None } else { Some(name) })
        })
    }

    /// Return the full name of this user.
    ///
    /// This is equal to the user's first name concatenated with the user's last name, if this
    /// is not empty. Otherwise, it equals the user's first name.
    pub fn full_name(&self) -> String {
        let first_name = self.first_name().unwrap_or_default();
        if let Some(last_name) = self.last_name() {
            let mut name = String::with_capacity(first_name.len() + 1 + last_name.len());
            name.push_str(first_name);
            name.push(' ');
            name.push_str(last_name);
            name
        } else {
            first_name.to_string()
        }
    }

    /// Return the public @username of this user, if any.
    ///
    /// The returned username does not contain the "@" prefix.
    ///
    /// Outside of the application, people may link to this user with one of Telegram's URLs, such
    /// as https://t.me/username.
    pub fn username(&self) -> Option<&str> {
        self.user().and_then(|u| u.username.as_deref())
    }

    /// Return collectible usernames of this chat, if any.
    ///
    /// The returned usernames do not contain the "@" prefix.
    ///
    /// Outside of the application, people may link to this user with one of its username, such
    /// as https://t.me/username.
    pub fn usernames(&self) -> Vec<&str> {
        self.user()
            .and_then(|u| u.usernames.as_deref())
            .map_or(Vec::new(), |usernames| {
                usernames
                    .iter()
                    .map(|username| match username {
                        tl::enums::Username::Username(username) => username.username.as_ref(),
                    })
                    .collect()
            })
    }

    /// Return the phone number of this user, if they are not a bot and their privacy settings
    /// allow you to see it.
    pub fn phone(&self) -> Option<&str> {
        self.user().and_then(|u| u.phone.as_deref())
    }

    /// Return the photo of this user, if any.
    pub fn photo(&self) -> Option<&tl::types::UserProfilePhoto> {
        match self.user().and_then(|u| u.photo.as_ref()) {
            Some(maybe_photo) => match maybe_photo {
                tl::enums::UserProfilePhoto::Empty => None,
                tl::enums::UserProfilePhoto::Photo(photo) => Some(photo),
            },
            None => None,
        }
    }

    /// Does this user represent the account that's currently logged in?
    pub fn is_self(&self) -> bool {
        // TODO if is_self is false, check in chat cache if id == ourself
        self.user().map(|u| u.is_self).unwrap_or(false)
    }

    /// Is this user in your account's contact list?
    pub fn contact(&self) -> bool {
        self.user().map(|u| u.contact).unwrap_or(false)
    }

    /// Is this user a mutual contact?
    ///
    /// Contacts are mutual if both the user of the current account and this user have eachother
    /// in their respective contact list.
    pub fn mutual_contact(&self) -> bool {
        self.user().map(|u| u.mutual_contact).unwrap_or(false)
    }

    /// Has the account of this user been deleted?
    pub fn deleted(&self) -> bool {
        self.user().map(|u| u.deleted).unwrap_or(false)
    }

    /// Is the current account a bot?
    ///
    /// Bot accounts are those created by [@BotFather](https://t.me/BotFather).
    pub fn is_bot(&self) -> bool {
        self.user().map(|u| u.bot).unwrap_or(false)
    }

    /// If the current user is a bot, does it have [privacy mode] enabled?
    ///
    /// * Bots with privacy enabled won't see messages in groups unless they are replied or the
    ///   command includes their name (`/command@bot`).
    /// * Bots with privacy disabled will be able to see all messages in a group.
    ///
    /// [privacy mode]: https://core.telegram.org/bots#privacy-mode
    pub fn bot_privacy(&self) -> bool {
        self.user().map(|u| !u.bot_chat_history).unwrap_or(false)
    }

    /// If the current user is a bot, can it be added to groups?
    pub fn bot_supports_chats(self) -> bool {
        self.user().map(|u| u.bot_nochats).unwrap_or(false)
    }

    /// Has the account of this user been verified?
    ///
    /// Verified accounts, such as [@BotFather](https://t.me/BotFather), have a special icon next
    /// to their names in official applications (commonly a blue starred checkmark).
    pub fn verified(&self) -> bool {
        self.user().map(|u| u.verified).unwrap_or(false)
    }

    /// Does this user have restrictions applied to their account?
    pub fn restricted(&self) -> bool {
        self.user().map(|u| u.restricted).unwrap_or(false)
    }

    /// If the current user is a bot, does it want geolocation information on inline queries?
    pub fn bot_inline_geo(&self) -> bool {
        self.user().map(|u| u.bot_inline_geo).unwrap_or(false)
    }

    /// Is this user an official member of the support team?
    pub fn support(&self) -> bool {
        self.user().map(|u| u.support).unwrap_or(false)
    }

    /// Has this user been flagged for trying to scam other people?
    pub fn scam(&self) -> bool {
        self.user().map(|u| u.scam).unwrap_or(false)
    }

    /// The reason(s) why this user is restricted, could be empty.
    pub fn restriction_reason(&self) -> Vec<RestrictionReason> {
        if let Some(reasons) = self.user().and_then(|u| u.restriction_reason.as_ref()) {
            reasons.iter().map(RestrictionReason::from_raw).collect()
        } else {
            Vec::new()
        }
    }

    /// Return the placeholder for inline queries if the current user is a bot and has said
    /// placeholder configured.
    pub fn bot_inline_placeholder(&self) -> Option<&str> {
        self.user()
            .and_then(|u| u.bot_inline_placeholder.as_deref())
    }

    /// Language code of the user, if any.
    pub fn lang_code(&self) -> Option<&str> {
        self.user().and_then(|u| u.lang_code.as_deref())
    }
}

impl From<User> for PackedChat {
    fn from(chat: User) -> Self {
        chat.pack()
    }
}

impl From<&User> for PackedChat {
    fn from(chat: &User) -> Self {
        chat.pack()
    }
}
