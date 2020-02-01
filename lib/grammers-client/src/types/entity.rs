use grammers_tl_types as tl;

pub enum Entity {
    User(tl::types::User),
    Chat(tl::types::Chat),
    Channel(tl::types::Channel),
}

impl Entity {
    pub fn id(&self) -> i32 {
        match self {
            Self::User(user) => user.id,
            Self::Chat(chat) => chat.id,
            Self::Channel(channel) => channel.id,
        }
    }

    pub fn to_input_peer(&self) -> tl::enums::InputPeer {
        match self {
            Self::User(user) => tl::types::InputPeerUser {
                user_id: user.id,
                access_hash: user.access_hash.unwrap_or(0),
            }
            .into(),
            Self::Chat(chat) => tl::types::InputPeerChat { chat_id: chat.id }.into(),
            Self::Channel(channel) => tl::types::InputPeerChannel {
                channel_id: channel.id,
                access_hash: channel.access_hash.unwrap_or(0),
            }
            .into(),
        }
    }

    pub fn display(&self) -> String {
        match self {
            Self::User(user) => {
                if let Some(name) = &user.first_name {
                    name.clone()
                } else {
                    "Deleted Account".to_string()
                }
            }
            Self::Chat(chat) => chat.title.clone(),
            Self::Channel(channel) => channel.title.clone(),
        }
    }
}
