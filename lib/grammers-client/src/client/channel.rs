use super::Client;
use grammers_tl_types::{
    enums::{messages::ChatFull, InputChannel::Channel},
    types::InputChannel as InputChannel,
    functions::channels::GetFullChannel
};
use anyhow::{ Result, anyhow};

impl Client {
    pub async fn get_full_channel(&mut self, channel_id: i64) -> Result<ChatFull> {
        let input_channel = InputChannel { channel_id, access_hash: 0i64};
        let chat_full = self.invoke(&GetFullChannel { channel: Channel(input_channel) })
            .await
            .map_err(|e| anyhow!("get full channel error: {}", e.to_string()))?;

        Ok(chat_full)
    }
}