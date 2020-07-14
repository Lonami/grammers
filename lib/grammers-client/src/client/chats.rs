use crate::Client;
pub use grammers_mtsender::{AuthorizationError, InvocationError};
use grammers_tl_types as tl;

impl Client {
    /// Resolves a username into the user that owns it, if any.
    pub async fn resolve_username(
        &mut self,
        username: &str,
    ) -> Result<Option<tl::types::User>, InvocationError> {
        let tl::enums::contacts::ResolvedPeer::Peer(tl::types::contacts::ResolvedPeer {
            peer,
            users,
            ..
        }) = self
            .invoke(&tl::functions::contacts::ResolveUsername {
                username: username.into(),
            })
            .await?;

        match peer {
            tl::enums::Peer::User(tl::types::PeerUser { user_id }) => {
                return Ok(users
                    .into_iter()
                    .filter_map(|user| match user {
                        tl::enums::User::User(user) => {
                            if user.id == user_id {
                                Some(user)
                            } else {
                                None
                            }
                        }
                        tl::enums::User::Empty(_) => None,
                    })
                    .next());
            }
            tl::enums::Peer::Chat(_) => {}
            tl::enums::Peer::Channel(_) => {}
        }

        Ok(None)
    }
}
