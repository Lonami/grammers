mod deserializable;
mod generated;
mod serializable;

pub use deserializable::Deserializable;
pub use generated::{enums, functions, types};
pub use serializable::Serializable;

/// Wrapper type around a byte string with specialized deserialization.
pub struct Bytes(pub Vec<u8>);

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
    type Return;
}
