use super::Client;
use grammers_tl_types as tl;

const MIN_CHANNEL_ID: i64 = -1002147483647;
const MAX_CHANNEL_ID: i64 = -1000000000000;
const MIN_CHAT_ID: i64 = -2147483647;
const MAX_USER_ID_OLD: i64 = 2147483647;
const MAX_USER_ID: i64 = 999999999999;

enum PeerType {
    Chat,
    Channel,
    User,
}

impl Client {

    pub async fn get_chat(&mut self, chat_id: &str) -> anyhow::Result<()> {
        match chat_id.parse::<i64>() {
            Ok(peer_id) => {
                match Self::get_peer_type(peer_id)? {
                    PeerType::User => {
                        let input_user = tl::types::InputUser {
                            user_id: peer_id,
                            access_hash: 0i64,
                        };
                        let ids = vec![tl::enums::InputUser::User(input_user)];
                        let s = self.invoke(&tl::functions::users::GetUsers { id: ids }).await?;
                    },
                    PeerType::Chat => {
                        let s = self.invoke(&tl::functions::messages::GetChats { id: vec![peer_id] }).await?;
                    },
                    PeerType::Channel => {
                        let input_channel = tl::types::InputChannel { channel_id: peer_id, access_hash: 0i64};
                        let ids = vec![tl::enums::InputChannel::Channel(input_channel)];
                        let s = self.invoke(&tl::functions::channels::GetChannels { id: ids }).await?;
                    }
                }
            },
            Err(_) => {}
        }

        Ok(())
    }

    fn get_channel_id(peer_id: i64) {}

    fn get_peer_type(peer_id: i64) -> anyhow::Result<PeerType> {
        if peer_id < 0 {
            if MIN_CHAT_ID <= peer_id {
                return Ok(PeerType::Chat);
            }

            if MIN_CHANNEL_ID <= peer_id && peer_id < MAX_CHANNEL_ID {
                return Ok(PeerType::Channel);
            }
        }else if 0 < peer_id && peer_id <= MAX_USER_ID {
            return Ok(PeerType::User);
        }
        return Err(anyhow::anyhow!("Peer id invalid: {}", peer_id))
    }
}