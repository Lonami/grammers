//! This crate provides the [`Session`] trait, which allows client instances
//! to configure themselves with it.
//!
//! Sessions are used to persist the information required to connect to
//! Telegram to disk, so that the expensive process of initializing the
//! connection has only to be done once.
//!
//! [`Session`]: trait.session.html

mod session;
mod text_session;

pub use session::Session;
pub use text_session::TextSession;
