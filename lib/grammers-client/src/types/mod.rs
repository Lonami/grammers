// Copyright 2020 - developers of the `grammers` project.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// https://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or https://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

//! Custom types extending those provided by Telegram.
//!
//! A lot of fields in the types exported from this module are currently public even though
//! they directly uses `grammers-tl-types`. This will probably change before the 1.0 release.
pub mod attributes;
pub mod button;
pub mod callback_query;
pub mod chat;
pub mod chat_map;
pub mod chats;
pub mod dialog;
pub mod downloadable;
pub mod inline;
pub mod input_message;
pub mod iter_buffer;
pub mod login_token;
pub mod media;
pub mod message;
pub mod message_deletion;
pub mod participant;
pub mod password_token;
pub mod permissions;
pub mod photo_sizes;
pub mod reactions;
pub mod reply_markup;
pub mod terms_of_service;
pub mod update;

pub use attributes::Attribute;
pub use callback_query::CallbackQuery;
pub use chat::{Channel, Chat, Group, PackedChat, Platform, RestrictionReason, User};
pub use chat_map::ChatMap;
pub(crate) use chat_map::Peer;
pub use chats::{AdminRightsBuilder, BannedRightsBuilder};
pub use dialog::Dialog;
pub use downloadable::{ChatPhoto, Downloadable, UserProfilePhoto};
pub use inline::query::InlineQuery;
pub use inline::send::InlineSend;
pub use input_message::InputMessage;
pub use iter_buffer::IterBuffer;
pub use login_token::LoginToken;
pub(crate) use media::Uploaded;
pub use media::{Media, Photo};
pub use message::Message;
pub use message_deletion::MessageDeletion;
pub use participant::{Participant, Role};
pub use password_token::PasswordToken;
pub use permissions::{Permissions, Restrictions};
pub use reactions::InputReactions;
pub(crate) use reply_markup::ReplyMarkup;
pub use terms_of_service::TermsOfService;
pub use update::Update;
