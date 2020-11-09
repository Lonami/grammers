use grammers_tl_types as tl;

/// Extensions for making working with messages easier.
pub trait MessageExt {
    /// Get the `Peer` chat where this message was sent to.
    fn chat(&self) -> tl::enums::Peer;
}

impl MessageExt for tl::types::Message {
    fn chat(&self) -> tl::enums::Peer {
        self.peer_id.clone()
    }
}
