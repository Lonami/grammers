#![deny(unsafe_code)]

mod chat;
mod generated;
mod message_box;
mod storages;

pub use chat::{ChatHashCache, PackedChat, PackedType};
pub use generated::LAYER as VERSION;
pub use generated::enums::DataCenter;
pub use generated::types::UpdateState;
pub use generated::types::User;
pub use message_box::PrematureEndReason;
pub use message_box::{Gap, MessageBox, MessageBoxes, State, UpdatesLike, peer_from_input_peer};
pub use storages::{
    KNOWN_DC_OPTIONS, TlSession as Session, state_to_update_state, try_push_channel_state,
};

// Needed for auto-generated definitions.
use generated::{enums, types};
use grammers_tl_types::{Deserializable, Identifiable, Serializable, deserialize};
