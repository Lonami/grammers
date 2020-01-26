use std::io::{Read, Result};

pub trait Deserializable {
    /// Serializes the type from a given buffer.
    fn deserialize<B: Read>(buf: &mut B) -> Result<Self>
    where
        Self: std::marker::Sized;
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

impl Deserializable for crate::Bytes {
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

        Ok(crate::Bytes(result))
    }
}
