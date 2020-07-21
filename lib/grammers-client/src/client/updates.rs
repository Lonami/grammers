use crate::types::EntitySet;
use crate::Client;
use futures::stream::StreamExt;
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
    pub async fn next_updates<'a, 'b>(&'a mut self) -> Option<(UpdateIter, EntitySet<'b>)> {
        // FIXME when creating a client and logging in we get our own id which we should
        // persist, that means we shouldn't need to fetch it anywhere ever but we don't
        // do that yet. this also means receiving an update before sign in fails
        if self.user_id.is_none() {
            self.user_id = Some(match self.get_me().await {
                Ok(me) => me.id,
                Err(_) => return None,
            });
        }

        use tl::enums::Updates::*;

        loop {
            break match self.next_raw_updates().await? {
                UpdateShort(update) => {
                    Some((UpdateIter::single(update.update), EntitySet::empty()))
                }
                Combined(update) => Some((
                    UpdateIter::multiple(update.updates),
                    EntitySet::new_owned(update.users, update.chats),
                )),
                Updates(update) => Some((
                    UpdateIter::multiple(update.updates),
                    EntitySet::new_owned(update.users, update.chats),
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
                                from_id: Some(if update.out {
                                    self.user_id.unwrap()
                                } else {
                                    update.user_id
                                }),
                                to_id: tl::enums::Peer::User(tl::types::PeerUser {
                                    user_id: if update.out {
                                        update.user_id
                                    } else {
                                        self.user_id.unwrap()
                                    },
                                }),
                                fwd_from: update.fwd_from,
                                via_bot_id: update.via_bot_id,
                                reply_to_msg_id: update.reply_to_msg_id,
                                date: update.date,
                                message: update.message,
                                media: None,
                                reply_markup: None,
                                entities: update.entities,
                                views: None,
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
                                from_id: Some(update.from_id),
                                to_id: tl::enums::Peer::Chat(tl::types::PeerChat {
                                    chat_id: update.chat_id,
                                }),
                                fwd_from: update.fwd_from,
                                via_bot_id: update.via_bot_id,
                                reply_to_msg_id: update.reply_to_msg_id,
                                date: update.date,
                                message: update.message,
                                media: None,
                                reply_markup: None,
                                entities: update.entities,
                                views: None,
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
        }
    }

    /// Provides access to the next `Updates` in the same way they arrive from Telegram.
    ///
    /// Working directly with these is a bit more involved than using the method `next_update`,
    /// because they may be nested (containing either a single update or multiple), or be the
    /// update itself but belonging to a different enumeration.
    ///
    /// Similar using an iterator manually, this method will return `Some` until no more updates
    /// are available (e.g. a disconnection occurred).
    pub async fn next_raw_updates(&mut self) -> Option<tl::enums::Updates> {
        self.updates.next().await
    }
}
