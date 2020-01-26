mod deserializable;
mod generated;
mod serializable;

pub use deserializable::Deserializable;
pub use generated::{enums, functions, types};
pub use serializable::Serializable;

/// Wrapper type around a vector of bytes with specialized deserialization.
pub struct Bytes(Vec<u8>);

/// Anything implementing this trait is identifiable by both ends (client-server)
/// when performing Remote Procedure Calls (RPC) and transmission of objects.
pub trait Identifiable {
    /// The unique identifier for the type.
    const CONSTRUCTOR_ID: u32;
}

pub trait RPC: Serializable {
    type Return;
}
