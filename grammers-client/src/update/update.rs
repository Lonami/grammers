// Copyright 2020 - developers of the `grammers` project.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// https://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or https://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

use grammers_session::updates::State;
use grammers_tl_types as tl;

use super::{CallbackQuery, InlineQuery, InlineSend, Message, MessageDeletion, Raw};
use crate::{Client, peer::PeerMap, utils};

/// An update that indicates some event, which may be of interest to the logged-in account, has occured.
///
/// Only updates pertaining to messages are guaranteed to be delivered, and can be fetched on-demand if
/// they occured while the client was offline by enabling [`catch_up`](crate::client::UpdatesConfiguration::catch_up).
#[non_exhaustive]
#[derive(Debug, Clone)]
pub enum Update {
    /// Occurs whenever a new text message or a message with media is produced.
    NewMessage(Message),
    /// Occurs when a message is updated.
    MessageEdited(Message),
    /// Occurs when a message is deleted.
    MessageDeleted(MessageDeletion),
    /// Occurs when Telegram calls back into your bot because an inline callback
    /// button was pressed.
    CallbackQuery(CallbackQuery),
    /// Occurs whenever you sign in as a bot and a user sends an inline query
    /// such as `@bot query`.
    InlineQuery(InlineQuery),
    /// Represents an update of user choosing the result of inline query and sending it to their peer partner.
    InlineSend(InlineSend),
    /// Raw events are not actual events.
    /// Instead, they are the raw Update object that Telegram sends. You
    /// normally shouldn’t need these.
    ///
    /// <div class="warning">The library can "move" raw update types into
    /// a separate update wrapper variant during minor version bumps, so use
    /// this only as a workaround when such variant is not available yet.</div>
    Raw(Raw),
}

impl Update {
    /// Create new friendly to use `Update`` from its raw version and peer map.
    pub fn from_raw(
        client: &Client,
        update: tl::enums::Update,
        state: State,
        peers: PeerMap,
    ) -> Self {
        match &update {
            // NewMessage
            tl::enums::Update::NewMessage(raw) => {
                if utils::peer_from_message(&raw.message).is_none() {
                    return Self::Raw(Raw { raw: update, state });
                }

                Self::NewMessage(Message {
                    msg: crate::message::Message::from_raw(
                        client,
                        raw.message.clone(),
                        None,
                        peers,
                    ),
                    raw: update,
                    state,
                })
            }

            tl::enums::Update::NewChannelMessage(raw) => {
                if utils::peer_from_message(&raw.message).is_none() {
                    return Self::Raw(Raw { raw: update, state });
                }

                Self::NewMessage(Message {
                    msg: crate::message::Message::from_raw(
                        client,
                        raw.message.clone(),
                        None,
                        peers,
                    ),
                    raw: update,
                    state,
                })
            }

            // MessageEdited
            tl::enums::Update::EditMessage(raw) => {
                if utils::peer_from_message(&raw.message).is_none() {
                    return Self::Raw(Raw { raw: update, state });
                }

                Self::MessageEdited(Message {
                    msg: crate::message::Message::from_raw(
                        client,
                        raw.message.clone(),
                        None,
                        peers,
                    ),
                    raw: update,
                    state,
                })
            }
            tl::enums::Update::EditChannelMessage(raw) => Self::MessageEdited(Message {
                msg: crate::message::Message::from_raw(client, raw.message.clone(), None, peers),
                raw: update,
                state,
            }),

            // MessageDeleted
            tl::enums::Update::DeleteMessages(_) => {
                Self::MessageDeleted(MessageDeletion { raw: update, state })
            }
            tl::enums::Update::DeleteChannelMessages(_) => {
                Self::MessageDeleted(MessageDeletion { raw: update, state })
            }

            // CallbackQuery
            tl::enums::Update::BotCallbackQuery(_) => Self::CallbackQuery(CallbackQuery {
                raw: update,
                state,
                client: client.clone(),
                peers,
            }),

            // InlineCallbackQuery
            tl::enums::Update::InlineBotCallbackQuery(_) => Self::CallbackQuery(CallbackQuery {
                raw: update,
                state,
                client: client.clone(),
                peers,
            }),

            // InlineQuery
            tl::enums::Update::BotInlineQuery(_) => Self::InlineQuery(InlineQuery {
                raw: update,
                state,
                client: client.clone(),
                peers,
            }),

            // InlineSend
            tl::enums::Update::BotInlineSend(_) => Self::InlineSend(InlineSend {
                raw: update,
                state,
                client: client.clone(),
                peers,
            }),

            // Raw
            _ => Self::Raw(Raw { raw: update, state }),
        }
    }

    /// Update state.
    pub fn state(&self) -> &State {
        match self {
            Update::NewMessage(update) => &update.state,
            Update::MessageEdited(update) => &update.state,
            Update::MessageDeleted(update) => &update.state,
            Update::CallbackQuery(update) => &update.state,
            Update::InlineQuery(update) => &update.state,
            Update::InlineSend(update) => &update.state,
            Update::Raw(update) => &update.state,
        }
    }

    /// Raw update, as sent by Telegram.
    ///
    /// Only contains the individual [`Update`](tl::enums::Update),
    /// not the [`Updates`](tl::enums::Updates) container from which it may have come from.
    pub fn raw(&self) -> &tl::enums::Update {
        match self {
            Update::NewMessage(update) => &update.raw,
            Update::MessageEdited(update) => &update.raw,
            Update::MessageDeleted(update) => &update.raw,
            Update::CallbackQuery(update) => &update.raw,
            Update::InlineQuery(update) => &update.raw,
            Update::InlineSend(update) => &update.raw,
            Update::Raw(update) => &update.raw,
        }
    }
}
