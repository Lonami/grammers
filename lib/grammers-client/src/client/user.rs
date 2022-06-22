use super::Client;
use grammers_mtsender::InvocationError;
use grammers_tl_types as tl;

impl Client {
    pub async fn get_full_user(
        &mut self,
        user_id: i64,
    ) -> Result<tl::enums::users::UserFull, InvocationError> {
        let input_user = tl::types::InputUser {
            user_id,
            access_hash: 0i64,
        };
        let user_full = self
            .invoke(&tl::functions::users::GetFullUser {
                id: tl::enums::InputUser::User(input_user),
            })
            .await?;

        Ok(user_full)
    }
}
