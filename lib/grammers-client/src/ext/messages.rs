use grammers_tl_types as tl;

/// Extensions for making working with messages easier.
pub trait MessageExt {
    /// Get the `Peer` chat where this message was sent to.
    fn chat(&self) -> tl::enums::Peer;
}

impl MessageExt for tl::types::Message {
    fn chat(&self) -> tl::enums::Peer {
        if !self.out && matches!(self.to_id, tl::enums::Peer::User(_)) {
            // Sent in private, `to_id` is us, build peer from `from_id` instead
            tl::enums::Peer::User(tl::types::PeerUser {
                user_id: self.from_id.unwrap(),
            })
        } else {
            self.to_id.clone()
        }
    }
}
