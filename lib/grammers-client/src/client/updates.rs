// Copyright 2020 - developers of the `grammers` project.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// https://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or https://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

//! Methods to deal with and offer access to updates.

use super::{Client, Step};
use crate::types::EntitySet;
use grammers_mtsender::ReadError;
pub use grammers_mtsender::{AuthorizationError, InvocationError};
use grammers_tl_types as tl;

pub enum UpdateIter {
    Single(Option<tl::enums::Update>),
    Multiple(Vec<tl::enums::Update>),
}

impl UpdateIter {
    fn single(update: tl::enums::Update) -> Self {
        Self::Single(Some(update))
    }

    fn multiple(mut updates: Vec<tl::enums::Update>) -> Self {
        updates.reverse();
        Self::Multiple(updates)
    }
}

impl Iterator for UpdateIter {
    type Item = tl::enums::Update;

    fn next(&mut self) -> Option<Self::Item> {
        match self {
            UpdateIter::Single(update) => update.take(),
            UpdateIter::Multiple(updates) => updates.pop(),
        }
    }
}

impl Client {
    /// Returns an iterator with the last updates and some of the entities used in them
    /// in a set for easy access.
    ///
    /// Similar using an iterator manually, this method will return `Some` until no more updates
    /// are available (e.g. a disconnection occurred).
    pub async fn next_updates(&mut self) -> Result<Option<(UpdateIter, EntitySet)>, ReadError> {
        use tl::enums::Updates::*;

        Ok(loop {
            let mut updates = match self.step().await? {
                Step::Connected { updates } => updates,
                Step::Disconnected => break None,
            };

            if updates.len() == 0 {
                continue;
            } else if updates.len() != 1 {
                panic!("telegram returned more than 1 updates in 1 step");
            }
            break match updates.pop().unwrap() {
                UpdateShort(update) => {
                    Some((UpdateIter::single(update.update), EntitySet::empty()))
                }
                Combined(update) => Some((
                    UpdateIter::multiple(update.updates),
                    EntitySet::new(update.users, update.chats),
                )),
                Updates(update) => Some((
                    UpdateIter::multiple(update.updates),
                    EntitySet::new(update.users, update.chats),
                )),
                // We need to know our self identifier by now or this will fail.
                // These updates will only happen after we logged in so that's fine.
                UpdateShortMessage(update) => Some((
                    (UpdateIter::single(tl::enums::Update::NewMessage(
                        tl::types::UpdateNewMessage {
                            message: tl::enums::Message::Message(tl::types::Message {
                                out: update.out,
                                mentioned: update.mentioned,
                                media_unread: update.media_unread,
                                silent: update.silent,
                                post: false,
                                from_scheduled: false,
                                legacy: false,
                                edit_hide: false,
                                id: update.id,
                                from_id: Some(tl::enums::Peer::User(tl::types::PeerUser {
                                    user_id: if update.out {
                                        // This update can only arrive when logged in (user_id is Some).
                                        self.user_id().unwrap()
                                    } else {
                                        update.user_id
                                    },
                                })),
                                peer_id: tl::enums::Peer::User(tl::types::PeerUser {
                                    user_id: if update.out {
                                        update.user_id
                                    } else {
                                        // This update can only arrive when logged in (user_id is Some).
                                        self.user_id().unwrap()
                                    },
                                }),
                                fwd_from: update.fwd_from,
                                via_bot_id: update.via_bot_id,
                                reply_to: update.reply_to,
                                date: update.date,
                                message: update.message,
                                media: None,
                                reply_markup: None,
                                entities: update.entities,
                                views: None,
                                forwards: None,
                                replies: None,
                                edit_date: None,
                                post_author: None,
                                grouped_id: None,
                                restriction_reason: None,
                            }),
                            pts: update.pts,
                            pts_count: update.pts_count,
                        },
                    ))),
                    EntitySet::empty(),
                )),
                UpdateShortChatMessage(update) => Some((
                    (UpdateIter::single(tl::enums::Update::NewMessage(
                        tl::types::UpdateNewMessage {
                            message: tl::enums::Message::Message(tl::types::Message {
                                out: update.out,
                                mentioned: update.mentioned,
                                media_unread: update.media_unread,
                                silent: update.silent,
                                post: false,
                                from_scheduled: false,
                                legacy: false,
                                edit_hide: false,
                                id: update.id,
                                from_id: Some(tl::enums::Peer::User(tl::types::PeerUser {
                                    user_id: update.from_id,
                                })),
                                peer_id: tl::enums::Peer::Chat(tl::types::PeerChat {
                                    chat_id: update.chat_id,
                                }),
                                fwd_from: update.fwd_from,
                                via_bot_id: update.via_bot_id,
                                reply_to: update.reply_to,
                                date: update.date,
                                message: update.message,
                                media: None,
                                reply_markup: None,
                                entities: update.entities,
                                views: None,
                                forwards: None,
                                replies: None,
                                edit_date: None,
                                post_author: None,
                                grouped_id: None,
                                restriction_reason: None,
                            }),
                            pts: update.pts,
                            pts_count: update.pts_count,
                        },
                    ))),
                    EntitySet::empty(),
                )),
                // These shouldn't really occur unless triggered via a request
                TooLong => panic!("should not receive updatesTooLong via passive updates"),
                UpdateShortSentMessage(_) => {
                    panic!("should not receive updateShortSentMessage via passive updates")
                }
            };
        })
    }
}
