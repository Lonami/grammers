use super::Client;
use grammers_tl_types::{
    enums::messages::ChatFull,
    functions::messages::GetFullChat,
};
use anyhow::{ Result, anyhow};

impl Client {

    pub async fn get_full_chat(&mut self, chat_id: i64) -> Result<ChatFull> {
        let chat_full = self.invoke(&GetFullChat { chat_id })
            .await
            .map_err(|e| anyhow!("get full chat error: {}", e.to_string()))?;

        Ok(chat_full)
    }
}