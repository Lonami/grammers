mod generated;
pub use generated::{enums, functions, types};

use std::io::{Read, Result, Write};

/// Anything implementing this trait is identifiable by both ends (client-server)
/// when performing Remote Procedure Calls (RPC) and transmission of objects.
pub trait Identifiable {
    /// The unique identifier for the type.
    fn constructor_id() -> u32;
}

pub trait Serializable: Identifiable {
    fn serialize<B: Write>(&self, buf: &mut B) -> Result<()> {
        buf.write(&Self::constructor_id().to_le_bytes())?;
        self.serialize_body(buf)
    }

    fn serialize_body<B: Write>(&self, buf: &mut B) -> Result<()>;
}

pub trait Deserializable {
    fn deserialize<B: Read>(buf: &mut B) -> Result<Self>
    where
        Self: std::marker::Sized;
}

pub trait RPC: Serializable {
    type Return;
}
