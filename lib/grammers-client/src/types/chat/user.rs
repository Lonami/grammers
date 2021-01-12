// Copyright 2020 - developers of the `grammers` project.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// https://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or https://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.
use grammers_tl_types as tl;
use std::fmt;

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
pub struct User(tl::types::User);

impl fmt::Debug for User {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

impl User {
    pub(crate) fn from_raw(user: tl::enums::User) -> Self {
        Self(match user {
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
            },
            tl::enums::User::User(user) => user,
        })
    }

    pub(crate) fn to_peer(&self) -> tl::enums::Peer {
        tl::types::PeerUser { user_id: self.0.id }.into()
    }

    pub(crate) fn to_input_peer(&self) -> tl::enums::InputPeer {
        tl::types::InputPeerUser {
            user_id: self.0.id,
            access_hash: self.0.access_hash.unwrap_or(0),
        }
        .into()
    }

    pub(crate) fn to_input(&self) -> tl::enums::InputUser {
        tl::types::InputUser {
            user_id: self.0.id,
            access_hash: self.0.access_hash.unwrap_or(0),
        }
        .into()
    }

    /// Return the unique identifier for this user.
    pub fn id(&self) -> i32 {
        self.0.id
    }

    /// Return the first name of this user.
    ///
    /// If the account was deleted, the returned string will be empty.
    pub fn first_name(&self) -> &str {
        self.0.first_name.as_deref().unwrap_or("")
    }

    /// Return the last name of this user, if any.
    pub fn last_name(&self) -> Option<&str> {
        self.0
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
        self.0.username.as_deref()
    }

    /// Does this user represent the account that's currently logged in?
    pub fn is_self(&self) -> bool {
        // TODO if is_self is false, check in chat cache if id == ourself
        self.0.is_self
    }

    /// Is this user represent a bot account?
    pub fn bot(&self) -> bool {
        self.0.bot
    }
}
