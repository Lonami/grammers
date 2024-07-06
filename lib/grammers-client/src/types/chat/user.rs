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
    pub raw: tl::types::User,
}

impl fmt::Debug for User {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.raw.fmt(f)
    }
}

// TODO: photo
impl User {
    pub fn from_raw(user: tl::enums::User) -> Self {
        Self {
            raw: match user {
                tl::enums::User::Empty(empty) => tl::types::User {
                    is_self: false,
                    contact: false,
                    mutual_contact: false,
                    deleted: false,
                    bot: false,
                    bot_chat_history: false,
                    bot_nochats: false,
                    verified: false,
                    restricted: false,
                    min: false,
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
                    id: empty.id,
                    access_hash: None,
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
                    bot_business: false,
                },
                tl::enums::User::User(user) => user,
            },
        }
    }

    /// Return the user presence status (also known as "last seen").
    pub fn status(&self) -> &grammers_tl_types::enums::UserStatus {
        self.raw
            .status
            .as_ref()
            .unwrap_or(&grammers_tl_types::enums::UserStatus::Empty)
    }

    /// Return the unique identifier for this user.
    pub fn id(&self) -> i64 {
        self.raw.id
    }

    pub(crate) fn access_hash(&self) -> Option<i64> {
        self.raw.access_hash
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
    /// If the account was deleted, the returned string will be empty.
    pub fn first_name(&self) -> &str {
        self.raw.first_name.as_deref().unwrap_or("")
    }

    /// Return the last name of this user, if any.
    pub fn last_name(&self) -> Option<&str> {
        self.raw
            .last_name
            .as_deref()
            .and_then(|name| if name.is_empty() { None } else { Some(name) })
    }

    /// Return the full name of this user.
    ///
    /// This is equal to the user's first name concatenated with the user's last name, if this
    /// is not empty. Otherwise, it equals the user's first name.
    pub fn full_name(&self) -> String {
        let first_name = self.first_name();
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
        self.raw.username.as_deref()
    }

    /// Return the phone number of this user, if they are not a bot and their privacy settings
    /// allow you to see it.
    pub fn phone(&self) -> Option<&str> {
        self.raw.phone.as_deref()
    }

    /// Return the photo of this user, if any.
    pub fn photo(&self) -> Option<&tl::types::UserProfilePhoto> {
        match self.raw.photo.as_ref() {
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
        self.raw.is_self
    }

    /// Is this user in your account's contact list?
    pub fn contact(&self) -> bool {
        self.raw.contact
    }

    /// Is this user a mutual contact?
    ///
    /// Contacts are mutual if both the user of the current account and this user have eachother
    /// in their respective contact list.
    pub fn mutual_contact(&self) -> bool {
        self.raw.mutual_contact
    }

    /// Has the account of this user been deleted?
    pub fn deleted(&self) -> bool {
        self.raw.deleted
    }

    /// Is the current account a bot?
    ///
    /// Bot accounts are those created by [@BotFather](https://t.me/BotFather).
    pub fn is_bot(&self) -> bool {
        self.raw.bot
    }

    /// If the current user is a bot, does it have [privacy mode] enabled?
    ///
    /// * Bots with privacy enabled won't see messages in groups unless they are replied or the
    /// command includes their name (`/command@bot`).
    /// * Bots with privacy disabled will be able to see all messages in a group.
    ///
    /// [privacy mode]: https://core.telegram.org/bots#privacy-mode
    pub fn bot_privacy(&self) -> bool {
        !self.raw.bot_chat_history
    }

    /// If the current user is a bot, can it be added to groups?
    pub fn bot_supports_chats(self) -> bool {
        self.raw.bot_nochats
    }

    /// Has the account of this user been verified?
    ///
    /// Verified accounts, such as [@BotFather](https://t.me/BotFather), have a special icon next
    /// to their names in official applications (commonly a blue starred checkmark).
    pub fn verified(&self) -> bool {
        self.raw.verified
    }

    /// Does this user have restrictions applied to their account?
    pub fn restricted(&self) -> bool {
        self.raw.restricted
    }

    /// If the current user is a bot, does it want geolocation information on inline queries?
    pub fn bot_inline_geo(&self) -> bool {
        self.raw.bot_inline_geo
    }

    /// Is this user an official member of the support team?
    pub fn support(&self) -> bool {
        self.raw.support
    }

    /// Has this user been flagged for trying to scam other people?
    pub fn scam(&self) -> bool {
        self.raw.scam
    }

    /// The reason(s) why this user is restricted, could be empty.
    pub fn restriction_reason(&self) -> Vec<RestrictionReason> {
        if let Some(reasons) = &self.raw.restriction_reason {
            reasons.iter().map(RestrictionReason::from_raw).collect()
        } else {
            Vec::new()
        }
    }

    /// Return the placeholder for inline queries if the current user is a bot and has said
    /// placeholder configured.
    pub fn bot_inline_placeholder(&self) -> Option<&str> {
        self.raw.bot_inline_placeholder.as_deref()
    }

    /// Language code of the user, if any.
    pub fn lang_code(&self) -> Option<&str> {
        self.raw.lang_code.as_deref()
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
