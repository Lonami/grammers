// Copyright 2020 - developers of the `grammers` project.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// https://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or https://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

//! Methods related to sending messages.

use crate::{ext::MessageExt, types, ClientHandle};
pub use grammers_mtsender::{AuthorizationError, InvocationError};
use grammers_tl_types as tl;
use std::time::{SystemTime, UNIX_EPOCH};

/// Generate a random message ID suitable for `send_message`.
fn generate_random_message_id() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system time is before epoch")
        .as_nanos() as i64
}

impl ClientHandle {
    /// Sends a text message to the desired chat.
    // TODO don't require nasty InputPeer
    pub async fn send_message(
        &mut self,
        chat: tl::enums::InputPeer,
        message: types::Message,
    ) -> Result<(), InvocationError> {
        self.invoke(&tl::functions::messages::SendMessage {
            no_webpage: !message.link_preview,
            silent: message.silent,
            background: message.background,
            clear_draft: message.clear_draft,
            peer: chat,
            reply_to_msg_id: message.reply_to,
            message: message.text,
            random_id: generate_random_message_id(),
            reply_markup: message.reply_markup,
            entities: if message.entities.is_empty() {
                None
            } else {
                Some(message.entities)
            },
            schedule_date: message.schedule_date,
        })
        .await?;
        Ok(())
    }

    /// Edits an existing text message
    // TODO don't require nasty InputPeer
    // TODO Media
    pub async fn edit_message(
        &mut self,
        chat: tl::enums::InputPeer,
        message_id: i32,
        new_message: types::Message,
    ) -> Result<(), InvocationError> {
        self.invoke(&tl::functions::messages::EditMessage {
            no_webpage: !new_message.link_preview,
            peer: chat,
            id: message_id,
            message: Some(new_message.text),
            media: None,
            reply_markup: new_message.reply_markup,
            entities: Some(new_message.entities),
            schedule_date: new_message.schedule_date,
        })
        .await?;

        Ok(())
    }

    async fn a_reply_msg(
        &mut self,
        chat: &tl::enums::InputPeer,
        id: tl::enums::InputMessage,
    ) -> (Option<tl::enums::messages::Messages>, bool) {
        if let tl::enums::InputPeer::Channel(chan) = chat {
            (
                self.invoke(&tl::functions::channels::GetMessages {
                    id: vec![id],
                    channel: tl::enums::InputChannel::Channel(tl::types::InputChannel {
                        channel_id: chan.channel_id,
                        access_hash: chan.access_hash,
                    }),
                })
                .await
                .ok(),
                false,
            )
        } else {
            (
                self.invoke(&tl::functions::messages::GetMessages { id: vec![id] })
                    .await
                    .ok(),
                true,
            )
        }
    }

    /// Gets the reply to message of a message
    /// Throws NotFound error if there's no reply to message
    // TODO don't require nasty InputPeer
    pub async fn get_reply_to_message(
        &mut self,
        chat: tl::enums::InputPeer,
        message: &tl::types::Message,
    ) -> Option<tl::types::Message> {
        let input_id =
            tl::enums::InputMessage::ReplyTo(tl::types::InputMessageReplyTo { id: message.id });

        let (mut res, mut filter_req) = self.a_reply_msg(&chat, input_id).await;
        if res.is_none() {
            let input_id = tl::enums::InputMessage::Id(tl::types::InputMessageId {
                id: message.reply_to_message_id()?,
            });
            let r = self.a_reply_msg(&chat, input_id).await;
            res = r.0;
            filter_req = r.1;
        }

        let mut reply_msg_l = match res? {
            tl::enums::messages::Messages::Messages(m) => Some(m.messages),
            tl::enums::messages::Messages::Slice(m) => Some(m.messages),
            tl::enums::messages::Messages::ChannelMessages(m) => Some(m.messages),
            _ => None,
        }?;

        if filter_req {
            let chat = message.chat();
            return reply_msg_l
                .into_iter()
                .filter_map(|m| {
                    if let tl::enums::Message::Message(msg) = m {
                        Some(msg)
                    } else {
                        None
                    }
                })
                .filter(|m| m.chat() == chat)
                .next();
        } else {
            if let tl::enums::Message::Message(msg) = reply_msg_l.remove(0) {
                return Some(msg);
            } else {
                return None;
            }
        }
    }

    // TODO don't keep this, it should be implicit
    pub async fn input_peer_for_username(
        &mut self,
        username: &str,
    ) -> Result<tl::enums::InputPeer, InvocationError> {
        if username.eq_ignore_ascii_case("me") {
            Ok(tl::enums::InputPeer::PeerSelf)
        } else if let Some(user) = self.resolve_username(username).await? {
            Ok(tl::types::InputPeerUser {
                user_id: user.id,
                access_hash: user.access_hash.unwrap(), // TODO don't unwrap
            }
            .into())
        } else {
            // TODO same rationale as IntoInput<tl::enums::InputPeer> for tl::types::User
            todo!("user without username not handled")
        }
    }
}
