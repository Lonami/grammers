use crate::{types, Client};
pub use grammers_mtsender::{AuthorizationError, InvocationError};
use grammers_tl_types as tl;
use std::io;
use std::time::{SystemTime, UNIX_EPOCH};

/// Generate a random message ID suitable for `send_message`.
fn generate_random_message_id() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system time is before epoch")
        .as_nanos() as i64
}

impl Client {
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

    // TODO don't keep this, it should be implicit
    pub async fn input_peer_for_username(
        &mut self,
        username: &str,
    ) -> Result<tl::enums::InputPeer, InvocationError> {
        if username.eq_ignore_ascii_case("me") {
            Ok(tl::enums::InputPeer::PeerSelf(tl::types::InputPeerSelf {}))
        } else if let Some(user) = self.resolve_username(username).await? {
            Ok(tl::types::InputPeerUser {
                user_id: user.id,
                access_hash: user.access_hash.unwrap(), // TODO don't unwrap
            }
            .into())
        } else {
            // TODO same rationale as IntoInput<tl::enums::InputPeer> for tl::types::User
            Err(InvocationError::IO(io::Error::new(
                io::ErrorKind::NotFound,
                "no user has that username",
            )))
        }
    }
}
