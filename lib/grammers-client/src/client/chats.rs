//! Methods related to chats and entities.
use crate::ClientHandle;
pub use grammers_mtsender::{AuthorizationError, InvocationError};
use grammers_tl_types as tl;

impl ClientHandle {
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

        Ok(match peer {
            tl::enums::Peer::User(tl::types::PeerUser { user_id }) => {
                users.into_iter().find_map(|user| match user {
                    tl::enums::User::User(user) if user.id == user_id => Some(user),
                    tl::enums::User::User(_) | tl::enums::User::Empty(_) => None,
                })
            }
            tl::enums::Peer::Chat(_) | tl::enums::Peer::Channel(_) => None,
        })
    }

    /// Fetch full information about the currently logged-in user.
    pub async fn get_me(&mut self) -> Result<tl::types::User, InvocationError> {
        let mut res = self
            .invoke(&tl::functions::users::GetUsers {
                id: vec![tl::enums::InputUser::UserSelf],
            })
            .await?;

        if res.len() != 1 {
            panic!("fetching only one user should exactly return one user");
        }

        match res.pop().unwrap() {
            tl::enums::User::User(user) => Ok(user),
            tl::enums::User::Empty(_) => panic!("should not get empty user when fetching self"),
        }
    }
}
