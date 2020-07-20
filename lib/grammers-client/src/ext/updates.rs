use grammers_tl_types as tl;

/// Extensions for making working with updates easier.
pub trait UpdateExt {
    /// Extract the non-service message contained in this update, if any.
    fn message(&self) -> Option<&tl::types::Message>;

    /// Extract the service message contained in this update, if any.
    fn service_message(&self) -> Option<&tl::types::MessageService>;
}

fn get_message(update: &tl::enums::Update) -> Option<&tl::enums::Message> {
    match update {
        tl::enums::Update::NewMessage(tl::types::UpdateNewMessage { message, .. }) => Some(message),
        tl::enums::Update::NewChannelMessage(tl::types::UpdateNewChannelMessage {
            message,
            ..
        }) => Some(message),
        _ => None,
    }
}

impl UpdateExt for tl::enums::Update {
    fn message(&self) -> Option<&tl::types::Message> {
        match get_message(self) {
            Some(tl::enums::Message::Message(message)) => Some(message),
            _ => None,
        }
    }

    fn service_message(&self) -> Option<&tl::types::MessageService> {
        match get_message(self) {
            Some(tl::enums::Message::Service(message)) => Some(message),
            _ => None,
        }
    }
}
