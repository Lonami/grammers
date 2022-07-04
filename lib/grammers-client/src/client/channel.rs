use super::Client;
use grammers_mtsender::InvocationError;
use grammers_session::PackedChat;
use grammers_tl_types as tl;

impl Client {
    const MAX_PARTICIPANT_LIMIT: usize = 200;

    /// get full channel
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

    /// get chat members
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
            limit: Self::MAX_PARTICIPANT_LIMIT as i32,
            hash: 0,
        };
        let mut chat_members: Vec<tl::enums::User> = vec![];
        loop {
            let res = self.invoke(&request).await?;
            match res {
                tl::enums::channels::ChannelParticipants::Participants(p) => {
                    for elem in p.users {
                        chat_members.push(elem);
                    }
                    if request.offset >= p.count {
                        break;
                    } else {
                        if request.limit >= p.count {
                            break;
                        } else if (request.offset + request.limit) >= p.count {
                            break;
                        } else {
                            request.offset += request.limit;
                        }
                    }
                }
                tl::enums::channels::ChannelParticipants::NotModified => {}
            }
        }
        Ok(chat_members)
    }
}
