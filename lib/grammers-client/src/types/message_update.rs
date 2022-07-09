/// Occurs whenever a message is deleted.
/// Note that this event isn’t 100% reliable, since Telegram doesn’t always
/// notify the clients that a message was deleted.
///
/// When `MessageDeletion#channel_id` is Some, it means the message was deleted
/// from a channel.
#[derive(Debug, Clone)]
pub struct MessageDeletion {
    pub(crate) channel_id: Option<i64>,
    pub(crate) messages: Vec<i32>,
}

impl MessageDeletion {
    /// Creates a new `MessageDeletion` from a vector of message IDs that is
    /// deleted in a channel.
    pub(crate) fn new_with_channel(messages: Vec<i32>, channel: i64) -> Self {
        Self {
            channel_id: Some(channel),
            messages,
        }
    }

    /// Creates a new `MessageDeletion` from a vector of message IDs.
    pub(crate) fn new(messages: Vec<i32>) -> Self {
        Self {
            channel_id: None,
            messages,
        }
    }

    /// Returns the channel ID if the message was deleted from a channel.
    pub fn channel_id(&self) -> Option<i64> {
        self.channel_id
    }

    /// Returns the slice of message IDs that was deleted.
    pub fn messages(&self) -> &[i32] {
        &self.messages
    }

    /// Gain ownership of underlying Vec of message IDs that was deleted.
    pub fn into_messages(self) -> Vec<i32> {
        self.messages
    }
}
