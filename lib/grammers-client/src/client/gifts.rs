use grammers_mtsender::InvocationError;
use grammers_session::PackedChat;
use grammers_tl_types as tl;

use crate::{Client, types::IterBuffer};

impl
    IterBuffer<
        crate::grammers_tl_types::functions::payments::GetSavedStarGifts,
        crate::types::gift::Gift,
    >
{
    async fn fill_buffer(&mut self) -> Result<usize, InvocationError> {
        let tl::enums::payments::SavedStarGifts::Gifts(gifts) =
            self.client.invoke(&self.request).await?;

        let count = gifts.gifts.len();

        if count < self.request.limit as usize && gifts.next_offset.is_none() {
            self.limit = Some(count);
        }

        self.buffer.extend(
            gifts
                .gifts
                .into_iter()
                .map(|g| crate::types::gift::Gift::from(tl::types::SavedStarGift::from(g))),
        );

        if let Some(offset) = gifts.next_offset {
            self.request.offset = offset;
        };

        Ok(count)
    }
}

pub type GiftsIter = IterBuffer<
    crate::grammers_tl_types::functions::payments::GetSavedStarGifts,
    crate::types::gift::Gift,
>;

impl GiftsIter {
    pub async fn next(&mut self) -> Result<Option<crate::types::gift::Gift>, InvocationError> {
        if let Some(data) = self.next_raw() {
            return data;
        }

        self.fill_buffer().await?;

        Ok(self.pop_item())
    }
}

impl Client {
    pub async fn get_available_gifts(
        &self,
    ) -> Result<Vec<crate::types::gift::Gift>, InvocationError> {
        let tl::enums::payments::StarGifts::Gifts(gifts) = self
            .invoke(&tl::functions::payments::GetStarGifts { hash: 0 })
            .await?
        else {
            return Ok(vec![]);
        };

        Ok(gifts
            .gifts
            .into_iter()
            .map(|gift| crate::types::gift::Gift::from(gift))
            .collect())
    }

    pub async fn get_gift_by_slug<T: Into<String>>(
        &self,
        slug: T,
    ) -> Result<crate::types::gift::Gift, InvocationError> {
        let s = self
            .invoke(&tl::functions::payments::GetUniqueStarGift { slug: slug.into() })
            .await?;

        Ok(crate::types::gift::Gift::from(s))
    }

    pub fn iter_gifts<C: Into<PackedChat>>(
        &self,
        chat_id: C,
        exclude_unsaved: bool,
        exclude_saved: bool,
        exclude_unlimited: bool,
        exclude_limited: bool,
        exclude_unique: bool,
        sort_by_value: bool,
        limit: Option<i32>,
        offset: String,
    ) -> GiftsIter {
        let chat: PackedChat = chat_id.into();
        let total = limit.unwrap_or(i32::MAX);
        let limit = total.min(100);

        IterBuffer::from_request(
            self,
            200,
            crate::grammers_tl_types::functions::payments::GetSavedStarGifts {
                peer: chat.to_input_peer(),
                exclude_limited,
                exclude_saved,
                exclude_unique,
                exclude_unlimited,
                exclude_unsaved,
                sort_by_value,
                collection_id: None,
                limit,
                offset,
            },
        )
    }
}
