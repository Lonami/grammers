#![deny(unsafe_code)]

mod chat;
mod dc_options;
mod generated;
mod message_box;
mod peer;
mod session;
pub mod storages;

pub use chat::{ChatHashCache, PackedChat, PackedType};
pub use dc_options::{DEFAULT_DC, KNOWN_DC_OPTIONS};
pub use generated::LAYER as VERSION;
pub use generated::enums::DataCenter;
pub use generated::types::User;
pub use message_box::PrematureEndReason;
pub use message_box::{Gap, MessageBox, MessageBoxes, State, UpdatesLike, peer_from_input_peer};
pub use peer::{ChannelKind, Peer, PeerInfo, PeerKind};
pub use session::{ChannelState, DcOption, Session, UpdateState, UpdatesState};

// Needed for auto-generated definitions.
use generated::{enums, types};
use grammers_tl_types::{Deserializable, Identifiable, Serializable, deserialize};
