use super::Client;
use grammers_mtsender::InvocationError;
use grammers_session::PackedChat;
use grammers_tl_types as tl;

const MAX_PARTICIPANT_LIMIT: i32 = 200;

impl Client {
    //get full channel
    pub async fn get_full_channel<C: Into<PackedChat>>(
        &mut self,
        chat: C,
    ) -> Result<tl::enums::messages::ChatFull, InvocationError> {
        let chat = chat.into();
        let input_channel = tl::types::InputChannel {
            channel_id: chat.id,
            access_hash: chat.access_hash.unwrap_or(0i64),
        };
        let chat_full = self
            .invoke(&tl::functions::channels::GetFullChannel {
                channel: tl::enums::InputChannel::Channel(input_channel),
            })
            .await?;

        Ok(chat_full)
    }

    // get chat' members
    pub async fn get_chat_members<C: Into<PackedChat>>(
        &self,
        chat: C,
        filter: tl::enums::ChannelParticipantsFilter,
    ) -> Result<Vec<tl::enums::User>, InvocationError> {
        let chat = chat.into();
        let input_channel = tl::types::InputChannel {
            channel_id: chat.id,
            access_hash: chat.access_hash.unwrap_or(0i64),
        };

        let mut request = tl::functions::channels::GetParticipants {
            channel: tl::enums::InputChannel::Channel(input_channel),
            filter,
            offset: 0,
            limit: MAX_PARTICIPANT_LIMIT,
            hash: 0,
        };
        let mut chat_members: Vec<tl::enums::User> = vec![];
        loop {
            if let tl::enums::channels::ChannelParticipants::Participants(p) =
                self.invoke(&request).await?
            {
                for elem in p.users {
                    chat_members.push(elem);
                }
                if request.offset >= p.count {
                    break;
                }

                if request.limit >= p.count {
                    break;
                }

                if (request.offset + request.limit) >= p.count {
                    break;
                }
                request.offset += request.limit;
            }
        }
        Ok(chat_members)
    }
}
