use grammers_tl_types as tl;

#[derive(Debug)]
pub struct InputUser(pub(crate) tl::enums::InputUser);

impl InputUser {
    pub fn _from_raw(raw: tl::enums::InputUser) -> Self {
        Self(raw)
    }

    pub fn _from_message(peer: tl::enums::InputPeer, msg_id: i32, user_id: i64) -> Self {
        Self::_from_raw(
            tl::types::InputUserFromMessage {
                peer,
                msg_id,
                user_id,
            }
            .into(),
        )
    }

    pub fn is_empty(&self) -> bool {
        self.0 == tl::enums::InputUser::Empty
    }

    pub fn is_self(&self) -> bool {
        self.0 == tl::enums::InputUser::UserSelf
    }
}

#[cfg(feature = "unstable_raw")]
impl From<InputUser> for tl::enums::InputUser {
    fn from(input_user: InputUser) -> Self {
        input_user.0
    }
}
