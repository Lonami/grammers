use super::Client;
use grammers_tl_types::{
    enums::{users, InputUser::User},
    types::InputUser,
    functions::users::GetFullUser
};
use anyhow::{ Result, anyhow};

impl Client {
    pub async fn get_full_user(&mut self, user_id: i64) -> Result<users::UserFull> {
        let input_user = InputUser {
            user_id,
            access_hash: 0i64,
        };
        let user_full = self.invoke(&GetFullUser { id: User(input_user) })
            .await
            .map_err(|e| anyhow!("get full user error: {}", e.to_string()))?;

        Ok(user_full)
    }
}