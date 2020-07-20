//! Extension traits to make dealing with Telegram types more pleasant.
mod messages;
mod updates;

pub use messages::MessageExt;
pub use updates::UpdateExt;
