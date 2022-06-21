use super::Client;
use anyhow::{anyhow, Result};
use grammers_tl_types::{
    enums::{
        channels::ChannelParticipants::{NotModified, Participants},
        messages::ChatFull,
        ChannelParticipantsFilter,
        InputChannel::Channel,
        User,
    },
    functions::channels::{GetFullChannel, GetParticipants},
    types::InputChannel,
};

impl Client {
    const MAX_PARTICIPANT_LIMIT: usize = 200;

    pub async fn get_full_channel(&mut self, channel_id: i64) -> Result<ChatFull> {
        let input_channel = InputChannel {
            channel_id,
            access_hash: 0i64,
        };
        let chat_full = self
            .invoke(&GetFullChannel {
                channel: Channel(input_channel),
            })
            .await
            .map_err(|e| anyhow!("get full channel error: {}", e.to_string()))?;

        Ok(chat_full)
    }

    pub async fn get_chat_members(
        &self,
        channel_id: i64,
        filter: ChannelParticipantsFilter,
    ) -> Result<Vec<User>> {
        let input_channel = InputChannel {
            channel_id,
            access_hash: 0i64,
        };

        let mut request = GetParticipants {
            channel: Channel(input_channel),
            filter,
            offset: 0,
            limit: Self::MAX_PARTICIPANT_LIMIT as i32,
            hash: 0,
        };
        let mut chat_members: Vec<User> = vec![];
        loop {
            let res = self.invoke(&request).await?;
            match res {
                Participants(p) => {
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
                NotModified => {}
            }
        }
        Ok(chat_members)
    }
}
