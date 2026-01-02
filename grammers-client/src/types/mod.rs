// Copyright 2020 - developers of the `grammers` project.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// https://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or https://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

//! Custom types extending those provided by Telegram.
//!
//! Properties containing raw types are public and will either be called "raw" or prefixed with "raw_".\
//! Keep in mind that **these fields are not part of the semantic versioning guarantees**.
mod action;
mod attributes;
pub mod button;
pub(crate) mod chats;
mod dialog;
mod downloadable;
mod input_media;
mod input_message;
mod iter_buffer;
mod login_token;
mod media;
mod message;
mod participant;
mod password_token;
mod peer;
mod peer_map;
mod permissions;
mod photo_sizes;
mod reactions;
pub mod reply_markup;
pub mod update;

pub use action::ActionSender;
pub use attributes::Attribute;
pub use chats::{AdminRightsBuilder, BannedRightsBuilder};
pub use dialog::Dialog;
pub use downloadable::Downloadable;
pub use input_media::InputMedia;
pub use input_message::InputMessage;
pub use iter_buffer::IterBuffer;
pub use login_token::LoginToken;
pub(crate) use media::Uploaded;
pub use media::{ChatPhoto, Media, Photo};
pub use message::Message;
pub use participant::{Participant, Role};
pub use password_token::PasswordToken;
pub use peer::{Channel, Group, Peer, Platform, RestrictionReason, User};
pub use peer_map::PeerMap;
pub use permissions::{Permissions, Restrictions};
pub use photo_sizes::PhotoSize;
pub use reactions::InputReactions;
pub(crate) use reply_markup::ReplyMarkup;
pub use update::Update;
