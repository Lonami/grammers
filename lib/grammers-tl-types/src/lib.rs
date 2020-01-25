mod generated;
pub use generated::{enums, functions, types};

use std::io::{Read, Result, Write};

/// Anything implementing this trait is identifiable by both ends (client-server)
/// when performing Remote Procedure Calls (RPC) and transmission of objects.
pub trait Identifiable {
    /// The unique identifier for the type.
    fn constructor_id() -> u32;
}

pub trait Serializable {
    /// Serializes the body into the given buffer.
    fn serialize<B: Write>(&self, buf: &mut B) -> Result<()>;
}

pub trait Deserializable {
    /// Serializes the type from a given buffer.
    fn deserialize<B: Read>(buf: &mut B) -> Result<Self>
    where
        Self: std::marker::Sized;
}

pub trait RPC: Serializable {
    type Return;
}

impl Serializable for bool {
    fn serialize<B: Write>(&self, buf: &mut B) -> Result<()> {
        if *self { 0x997275b5u32 } else { 0xbc799737u32 }.serialize(buf)
    }
}
impl Serializable for i32 {
    fn serialize<B: Write>(&self, buf: &mut B) -> Result<()> {
        buf.write(&self.to_le_bytes()).map(drop)
    }
}
impl Serializable for u32 {
    fn serialize<B: Write>(&self, buf: &mut B) -> Result<()> {
        buf.write(&self.to_le_bytes()).map(drop)
    }
}
impl Serializable for i64 {
    fn serialize<B: Write>(&self, buf: &mut B) -> Result<()> {
        buf.write(&self.to_le_bytes()).map(drop)
    }
}
impl Serializable for f64 {
    fn serialize<B: Write>(&self, buf: &mut B) -> Result<()> {
        buf.write(&self.to_le_bytes()).map(drop)
    }
}
impl<T: Serializable> Serializable for Vec<T> {
    fn serialize<B: Write>(&self, buf: &mut B) -> Result<()> {
        0x1cb5c415u32.serialize(buf)?;
        (self.len() as i32).serialize(buf)?;
        for x in self {
            x.serialize(buf)?;
        }
        Ok(())
    }
}
impl Serializable for String {
    fn serialize<B: Write>(&self, buf: &mut B) -> Result<()> {
        self.as_bytes().serialize(buf)
    }
}
impl Serializable for Vec<u8> {
    fn serialize<B: Write>(&self, buf: &mut B) -> Result<()> {
        (&self[..]).serialize(buf)
    }
}
impl Serializable for &[u8] {
    fn serialize<B: Write>(&self, buf: &mut B) -> Result<()> {
        let len = if self.len() < 254 {
            buf.write(&[self.len() as u8])?;
            self.len() + 1
        } else {
            buf.write(&[
                254,
                ((self.len() >> 0) & 0xff) as u8,
                ((self.len() >> 8) & 0xff) as u8,
                ((self.len() >> 16) & 0xff) as u8,
            ])?;
            self.len()
        };
        let padding = (4 - (len % 4)) % 4;

        buf.write(self)?;
        for _ in 0..padding {
            buf.write(&[0])?;
        }
        Ok(())
    }
}

impl Deserializable for bool {
    fn deserialize<B: Read>(buf: &mut B) -> Result<Self> {
        let id = u32::deserialize(buf)?;
        match id {
            0x997275b5u32 => Ok(true),
            0xbc799737u32 => Ok(false),
            _ => unimplemented!("return error"),
        }
    }
}
impl Deserializable for u8 {
    fn deserialize<B: Read>(buf: &mut B) -> Result<Self> {
        let mut buffer = [0u8; 1];
        buf.read_exact(&mut buffer)?;
        Ok(buffer[0])
    }
}
impl Deserializable for i32 {
    fn deserialize<B: Read>(buf: &mut B) -> Result<Self> {
        let mut buffer = [0u8; 4];
        buf.read_exact(&mut buffer)?;
        Ok(Self::from_le_bytes(buffer))
    }
}
impl Deserializable for u32 {
    fn deserialize<B: Read>(buf: &mut B) -> Result<Self> {
        let mut buffer = [0u8; 4];
        buf.read_exact(&mut buffer)?;
        Ok(Self::from_le_bytes(buffer))
    }
}
impl Deserializable for i64 {
    fn deserialize<B: Read>(buf: &mut B) -> Result<Self> {
        let mut buffer = [0u8; 8];
        buf.read_exact(&mut buffer)?;
        Ok(Self::from_le_bytes(buffer))
    }
}
impl Deserializable for f64 {
    fn deserialize<B: Read>(buf: &mut B) -> Result<Self> {
        let mut buffer = [0u8; 8];
        buf.read_exact(&mut buffer)?;
        Ok(Self::from_le_bytes(buffer))
    }
}
impl<T: Deserializable> Deserializable for Vec<T> {
    fn deserialize<B: Read>(buf: &mut B) -> Result<Self> {
        let id = u32::deserialize(buf)?;
        if id != 0x1cb5c415u32 {
            unimplemented!("return error");
        }
        let len = u32::deserialize(buf)?;
        Ok((0..len)
            .map(|_| T::deserialize(buf))
            .collect::<Result<Vec<T>>>()?)
    }
}
impl Deserializable for String {
    fn deserialize<B: Read>(buf: &mut B) -> Result<Self> {
        Ok(String::from_utf8_lossy(&Vec::<u8>::deserialize(buf)?).into())
    }
}
/*
impl Deserializable for Vec<u8> {
    fn deserialize<B: Read>(buf: &mut B) -> Result<Self> {
        let first_byte = u8::deserialize(buf)?;
        let (len, padding) = if first_byte == 254 {
            let mut buffer = [0u8; 3];
            buf.read_exact(&mut buffer)?;
            let len = ((buffer[0] << 0) | (buffer[1] << 8) | (buffer[2] << 16)) as usize;
            (len, len % 4)
        } else {
            let len = first_byte as usize;
            (len, (len + 1) % 4)
        };

        let mut result = vec![0u8; len];
        buf.read_exact(&mut result)?;

        if padding > 0 {
            for _ in 0..(4 - padding) {
                u8::deserialize(buf)?;
            }
        }

        Ok(result)
    }
}
*/
