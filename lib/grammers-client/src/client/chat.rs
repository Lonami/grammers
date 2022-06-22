use super::Client;
use grammers_mtsender::InvocationError;
use grammers_tl_types as tl;

impl Client {
    pub async fn get_full_chat(
        &mut self,
        chat_id: i64,
    ) -> Result<tl::enums::messages::ChatFull, InvocationError> {
        let chat_full = self
            .invoke(&tl::functions::messages::GetFullChat { chat_id })
            .await?;

        Ok(chat_full)
    }
}
