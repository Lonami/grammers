// Copyright 2020 - developers of the `grammers` project.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// https://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or https://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.
use grammers_tl_types as tl;

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
}
