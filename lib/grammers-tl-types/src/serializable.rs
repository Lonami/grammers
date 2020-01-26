use std::io::{Result, Write};

pub trait Serializable {
    /// Serializes the body into the given buffer.
    fn serialize<B: Write>(&self, buf: &mut B) -> Result<()>;
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
        let len = if self.len() <= 253 {
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
