//! This library contains Telegram's types and functions as Rust code
//! in the form of `struct` and `enum`, which can be serialized into
//! bytes and deserialized from bytes.
//!
//! # Features
//!
//! The default feature set includes:
//!
//! * `tl-api`.
//!
//! The available features are:
//!
//! * `tl-api`: generates code for the `api.tl`.
//!   This is what high-level libraries often need.
//!
//! * `mtproto-api`: generates code for the `mtproto.tl`.
//!   Only useful for low-level libraries.
//!
//! * `deserializable-functions`: adds `impl Deserializable` for `functions`.
//!   This might be of interest for server implementations, which need to
//!   deserialize the client's requests.
//!
//! [Type Language]: https://core.telegram.org/mtproto/TL
//! [Binary Data Serialization]: https://core.telegram.org/mtproto/serialize
mod deserializable;
pub mod errors;
mod generated;
mod serializable;

pub use deserializable::Deserializable;
pub use generated::{enums, functions, types};
pub use serializable::Serializable;

/// This struct represents the concrete type of a vector, that is,
/// `vector` as opposed to the type `Vector`. This bare type is less
/// common, so instead of creating a enum for `Vector` wrapping `vector`
/// as Rust's `Vec` (as we would do with auto-generated code),
/// a new-type for `vector` is used instead.
pub struct RawVec<T>(pub Vec<T>);

/// Anything implementing this trait is identifiable by both ends (client-server)
/// when performing Remote Procedure Calls (RPC) and transmission of objects.
pub trait Identifiable {
    /// The unique identifier for the type.
    const CONSTRUCTOR_ID: u32;
}

/// Structures implementing this trait indicate that they are suitable for
/// use to perform Remote Procedure Calls (RPC), and are able to determine
/// what the type of the response will be.
pub trait RPC: Serializable {
    /// The type of the "return" value coming from the other end of the
    /// connection.
    type Return: Deserializable;
}
