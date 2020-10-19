use grammers_tl_types as tl;

/// Extensions for making working with messages easier.
pub trait MessageExt {
    /// Get the `Peer` chat where this message was sent to.
    fn chat(&self) -> tl::enums::Peer;
}

impl MessageExt for tl::types::Message {
    fn chat(&self) -> tl::enums::Peer {
        if !self.out && matches!(self.peer_id, tl::enums::Peer::User(_)) {
            // Sent in private, `to_id` is us, build peer from `from_id` instead
            self.from_id.clone().unwrap()
        } else {
            self.peer_id.clone()
        }
    }
}
