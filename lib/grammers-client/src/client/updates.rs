// Copyright 2020 - developers of the `grammers` project.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// https://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or https://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

//! Methods to deal with and offer access to updates.

use super::{Client, ClientHandle, Step};
use crate::types::{EntitySet, Update};
use grammers_mtsender::ReadError;
pub use grammers_mtsender::{AuthorizationError, InvocationError};
use grammers_tl_types as tl;
use std::collections::VecDeque;
use std::sync::Arc;

pub struct UpdateIter {
    client: ClientHandle,
    updates: VecDeque<tl::enums::Update>,
    entities: Arc<EntitySet>,
}

impl UpdateIter {
    pub(crate) fn new(
        client: ClientHandle,
        updates: Vec<tl::enums::Update>,
        entities: Arc<EntitySet>,
    ) -> Self {
        Self {
            client,
            updates: updates.into(),
            entities,
        }
    }
}

impl Iterator for UpdateIter {
    type Item = Update;

    fn next(&mut self) -> Option<Self::Item> {
        while let Some(update) = self.updates.pop_front() {
            if let Some(update) = Update::new(&self.client, update, &self.entities) {
                return Some(update);
            }
        }

        None
    }
}

impl Client {
    /// Returns an iterator with the last updates and some of the entities used in them
    /// in a set for easy access.
    ///
    /// Similar using an iterator manually, this method will return `Some` until no more updates
    /// are available (e.g. a disconnection occurred).
    pub async fn next_updates(&mut self) -> Result<Option<UpdateIter>, ReadError> {
        Ok(loop {
            let updates = match self.step().await? {
                Step::Connected { updates } => updates,
                Step::Disconnected => break None,
            };

            if !updates.is_empty() {
                let (updates, users, chats) = updates
                    .into_iter()
                    .map(|update| self.adapt_updates(update))
                    .fold((vec![], vec![], vec![]), |mut old, new| {
                        old.0.extend(new.0);
                        old.1.extend(new.1);
                        old.2.extend(new.2);
                        old
                    });

                break Some(UpdateIter::new(
                    self.handle(),
                    updates,
                    EntitySet::new(users, chats),
                ));
            }
        })
    }

    fn adapt_updates(
        &self,
        updates: tl::enums::Updates,
    ) -> (
        Vec<tl::enums::Update>,
        Vec<tl::enums::User>,
        Vec<tl::enums::Chat>,
    ) {
        use tl::enums::Updates::*;

        match updates {
            UpdateShort(u) => (vec![u.update], vec![], vec![]),
            Combined(u) => (u.updates, u.users, u.chats),
            Updates(u) => (u.updates, u.users, u.chats),
            // We need to know our self identifier by now or this will fail.
            // These updates will only happen after we logged in so that's fine.
            UpdateShortMessage(update) => (
                vec![tl::enums::Update::NewMessage(tl::types::UpdateNewMessage {
                    message: tl::enums::Message::Message(tl::types::Message {
                        out: update.out,
                        mentioned: update.mentioned,
                        media_unread: update.media_unread,
                        silent: update.silent,
                        post: false,
                        from_scheduled: false,
                        legacy: false,
                        edit_hide: false,
                        pinned: false,
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
                })],
                vec![],
                vec![],
            ),
            UpdateShortChatMessage(update) => (
                vec![tl::enums::Update::NewMessage(tl::types::UpdateNewMessage {
                    message: tl::enums::Message::Message(tl::types::Message {
                        out: update.out,
                        mentioned: update.mentioned,
                        media_unread: update.media_unread,
                        silent: update.silent,
                        post: false,
                        from_scheduled: false,
                        legacy: false,
                        edit_hide: false,
                        pinned: false,
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
                })],
                vec![],
                vec![],
            ),
            // These shouldn't really occur unless triggered via a request
            TooLong => panic!("should not receive updatesTooLong via passive updates"),
            UpdateShortSentMessage(_) => {
                panic!("should not receive updateShortSentMessage via passive updates")
            }
        }
    }
}
