// Copyright 2020 - developers of the `grammers` project.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// https://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or https://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.
use crate::errors::DeserializeError;

/// Re-implement `Cursor` to only work over in-memory buffers and greatly
/// narrow the possible error cases.
pub struct Cursor<'a> {
    buf: &'a [u8],
    pos: usize,
}

impl Cursor<'_> {
    fn read_byte(&mut self) -> Result<u8> {
        if self.pos < self.buf.len() {
            let byte = self.buf[self.pos];
            self.pos += 1;
            Ok(byte)
        } else {
            Err(DeserializeError::UnexpectedEof)
        }
    }

    fn read_exact(&mut self, buf: &mut [u8]) -> Result<()> {
        if self.pos + buf.len() > self.buf.len() {
            Err(DeserializeError::UnexpectedEof)
        } else {
            buf.copy_from_slice(&self.buf[self.pos..self.pos + buf.len()]);
            self.pos += buf.len();
            Ok(())
        }
    }

    fn read_to_end(&mut self, buf: &mut Vec<u8>) -> Result<usize> {
        buf.extend(&self.buf[self.pos..]);
        let old = self.pos;
        self.pos = self.buf.len();
        Ok(self.pos - old)
    }
}

/// The problem with being generic over `std::io::Read` is that it's
/// fallible, but in practice, we're always going to serialize in-memory,
/// so instead we just use a `[u8]` as our buffer.
pub(crate) type Buffer<'a, 'b> = &'a mut Cursor<'b>;
pub(crate) type Result<T> = std::result::Result<T, DeserializeError>;

/// This trait allows for data serialized according to the
/// [Binary Data Serialization] to be deserialized into concrete instances.
///
/// [Binary Data Serialization]: https://core.telegram.org/mtproto/serialize
pub trait Deserializable {
    /// Deserializes an instance of the type from a given buffer.
    fn deserialize(buf: Buffer) -> Result<Self>
    where
        Self: std::marker::Sized;

    /// Convenience function to deserialize an instance from a given buffer.
    ///
    /// # Examples
    ///
    /// ```
    /// use grammers_tl_types::Deserializable;
    ///
    /// assert_eq!(bool::from_bytes(&[0x37, 0x97, 0x79, 0xbc]).unwrap(), false);
    /// ```
    fn from_bytes(buf: &[u8]) -> Result<Self>
    where
        Self: std::marker::Sized,
    {
        Self::deserialize(&mut Cursor { buf, pos: 0 })
    }
}

impl Deserializable for bool {
    /// Deserializes a boolean according to the following definitions:
    ///
    /// * `boolFalse#bc799737 = Bool;` deserializes into `false`.
    /// * `boolTrue#997275b5 = Bool;` deserializes into `true`.
    ///
    /// # Examples
    ///
    /// ```
    /// use grammers_tl_types::Deserializable;
    ///
    /// assert_eq!(bool::from_bytes(&[0xb5, 0x75, 0x72, 0x99]).unwrap(), true);
    /// assert_eq!(bool::from_bytes(&[0x37, 0x97, 0x79, 0xbc]).unwrap(), false);
    /// ```
    #[allow(clippy::unreadable_literal)]
    fn deserialize(buf: Buffer) -> Result<Self> {
        let id = u32::deserialize(buf)?;
        match id {
            0x997275b5u32 => Ok(true),
            0xbc799737u32 => Ok(false),
            _ => Err(DeserializeError::UnexpectedConstructor { id }),
        }
    }
}

impl Deserializable for i32 {
    /// Deserializes a 32-bit signed integer according to the following
    /// definition:
    ///
    /// * `int ? = Int;`.
    ///
    /// # Examples
    ///
    /// ```
    /// use grammers_tl_types::Deserializable;
    ///
    /// assert_eq!(i32::from_bytes(&[0x00, 0x00, 0x00, 0x00]).unwrap(), 0i32);
    /// assert_eq!(i32::from_bytes(&[0x01, 0x00, 0x00, 0x00]).unwrap(), 1i32);
    /// assert_eq!(i32::from_bytes(&[0xff, 0xff, 0xff, 0xff]).unwrap(), -1i32);
    /// assert_eq!(i32::from_bytes(&[0xff, 0xff, 0xff, 0x7f]).unwrap(), i32::max_value());
    /// assert_eq!(i32::from_bytes(&[0x00, 0x00, 0x00, 0x80]).unwrap(), i32::min_value());
    /// ```
    fn deserialize(buf: Buffer) -> Result<Self> {
        let mut buffer = [0u8; 4];
        buf.read_exact(&mut buffer)?;
        Ok(Self::from_le_bytes(buffer))
    }
}

impl Deserializable for u32 {
    /// Deserializes a 32-bit unsigned integer according to the following
    /// definition:
    ///
    /// * `int ? = Int;`.
    ///
    /// # Examples
    ///
    /// ```
    /// use grammers_tl_types::Deserializable;
    ///
    /// assert_eq!(u32::from_bytes(&[0x00, 0x00, 0x00, 0x00]).unwrap(), 0u32);
    /// assert_eq!(u32::from_bytes(&[0x01, 0x00, 0x00, 0x00]).unwrap(), 1u32);
    /// assert_eq!(u32::from_bytes(&[0xff, 0xff, 0xff, 0xff]).unwrap(), u32::max_value());
    /// assert_eq!(u32::from_bytes(&[0x00, 0x00, 0x00, 0x00]).unwrap(), u32::min_value());
    /// ```
    fn deserialize(buf: Buffer) -> Result<Self> {
        let mut buffer = [0u8; 4];
        buf.read_exact(&mut buffer)?;
        Ok(Self::from_le_bytes(buffer))
    }
}

impl Deserializable for i64 {
    /// Deserializes a 64-bit signed integer according to the following
    /// definition:
    ///
    /// * `long ? = Long;`.
    ///
    /// # Examples
    ///
    /// ```
    /// use grammers_tl_types::Deserializable;
    ///
    /// assert_eq!(i64::from_bytes(&[0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0]).unwrap(), 0i64);
    /// assert_eq!(i64::from_bytes(&[0x1, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0]).unwrap(), 1i64);
    /// assert_eq!(i64::from_bytes(&[0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff]).unwrap(), (-1i64));
    /// assert_eq!(i64::from_bytes(&[0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0x7f]).unwrap(), i64::max_value());
    /// assert_eq!(i64::from_bytes(&[0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x80]).unwrap(), i64::min_value());
    /// ```
    fn deserialize(buf: Buffer) -> Result<Self> {
        let mut buffer = [0u8; 8];
        buf.read_exact(&mut buffer)?;
        Ok(Self::from_le_bytes(buffer))
    }
}

impl Deserializable for [u8; 16] {
    /// Deserializes the 128-bit integer according to the following
    /// definition:
    ///
    /// * `int128 4*[ int ] = Int128;`.
    ///
    /// # Examples
    ///
    /// ```
    /// use grammers_tl_types::Deserializable;
    ///
    /// let data = [1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16];
    ///
    /// assert_eq!(<[u8; 16]>::from_bytes(&data).unwrap(), data);
    /// ```
    fn deserialize(buf: Buffer) -> Result<Self> {
        let mut buffer = [0u8; 16];
        buf.read_exact(&mut buffer)?;
        Ok(buffer)
    }
}

impl Deserializable for [u8; 32] {
    /// Deserializes the 128-bit integer according to the following
    /// definition:
    ///
    /// * `int256 8*[ int ] = Int256;`.
    ///
    /// # Examples
    ///
    /// ```
    /// use grammers_tl_types::Deserializable;
    ///
    /// let data = [1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17,
    ///             18, 19, 20, 21, 22, 23, 24, 25, 26, 27, 28, 29, 30, 31, 32];
    ///
    /// assert_eq!(<[u8; 32]>::from_bytes(&data).unwrap(), data);
    /// ```
    fn deserialize(buf: Buffer) -> Result<Self> {
        let mut buffer = [0u8; 32];
        buf.read_exact(&mut buffer)?;
        Ok(buffer)
    }
}

impl Deserializable for f64 {
    /// Deserializes a 64-bit floating point according to the
    /// following definition:
    ///
    /// * `double ? = Double;`.
    ///
    /// # Examples
    ///
    /// ```
    /// use std::f64;
    /// use grammers_tl_types::Deserializable;
    ///
    /// assert_eq!(f64::from_bytes(&[0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0]).unwrap(), 0f64);
    /// assert_eq!(f64::from_bytes(&[0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0xf8, 0x3f]).unwrap(), 1.5f64);
    /// assert_eq!(f64::from_bytes(&[0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0xf8, 0xbf]).unwrap(), -1.5f64);
    /// assert_eq!(f64::from_bytes(&[0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0xf0, 0x7f]).unwrap(), f64::INFINITY);
    /// assert_eq!(f64::from_bytes(&[0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0xf0, 0xff]).unwrap(), f64::NEG_INFINITY);
    /// ```
    fn deserialize(buf: Buffer) -> Result<Self> {
        let mut buffer = [0u8; 8];
        buf.read_exact(&mut buffer)?;
        Ok(Self::from_le_bytes(buffer))
    }
}

impl<T: Deserializable> Deserializable for Vec<T> {
    /// Deserializes a vector of deserializable items according to the
    /// following definition:
    ///
    /// * `vector#1cb5c415 {t:Type} # [ t ] = Vector t;`.
    ///
    /// # Examples
    ///
    /// ```
    /// use grammers_tl_types::Deserializable;
    ///
    /// assert_eq!(Vec::<i32>::from_bytes(&[0x15, 0xc4, 0xb5, 0x1c, 0x0, 0x0, 0x0, 0x0]).unwrap(), Vec::new());
    /// assert_eq!(Vec::<i32>::from_bytes(&[0x15, 0xc4, 0xb5, 0x1c, 0x1, 0x0, 0x0, 0x0, 0x7f, 0x0, 0x0, 0x0]).unwrap(),
    ///            vec![0x7f_i32]);
    /// ```
    #[allow(clippy::unreadable_literal)]
    fn deserialize(buf: Buffer) -> Result<Self> {
        let id = u32::deserialize(buf)?;
        if id != 0x1cb5c415u32 {
            return Err(DeserializeError::UnexpectedConstructor { id });
        }
        let len = u32::deserialize(buf)?;
        Ok((0..len)
            .map(|_| T::deserialize(buf))
            .collect::<Result<Vec<T>>>()?)
    }
}

impl<T: Deserializable> Deserializable for crate::RawVec<T> {
    /// Deserializes a vector of deserializable items according to the
    /// following definition:
    ///
    /// * `vector#1cb5c415 {t:Type} # [ t ] = Vector t;`.
    ///
    /// # Examples
    ///
    /// ```
    /// use grammers_tl_types::{RawVec, Deserializable};
    ///
    /// assert_eq!(RawVec::<i32>::from_bytes(&[0x0, 0x0, 0x0, 0x0]).unwrap().0, Vec::<i32>::new());
    /// assert_eq!(RawVec::<i32>::from_bytes(&[0x1, 0x0, 0x0, 0x0, 0x7f, 0x0, 0x0, 0x0]).unwrap().0, vec![0x7f_i32]);
    /// ```
    fn deserialize(buf: Buffer) -> Result<Self> {
        let len = u32::deserialize(buf)?;
        Ok(Self(
            (0..len)
                .map(|_| T::deserialize(buf))
                .collect::<Result<Vec<T>>>()?,
        ))
    }
}

impl Deserializable for crate::Blob {
    /// Deserializes a blob by doing no parsing or interpretation.
    fn deserialize(buf: Buffer) -> Result<Self> {
        let mut result = Vec::new();
        buf.read_to_end(&mut result)?;
        Ok(Self(result))
    }
}

impl Deserializable for String {
    /// Deserializes a UTF-8 string according to the following definition:
    ///
    /// * `string ? = String;`.
    ///
    /// # Examples
    ///
    /// ```
    /// use grammers_tl_types::Deserializable;
    ///
    /// fn test_string(string: &str, prefix: &[u8], suffix: &[u8]) {
    ///    let bytes = {
    ///        let mut tmp = prefix.to_vec();
    ///        tmp.extend(string.as_bytes());
    ///        tmp.extend(suffix);
    ///        tmp
    ///    };
    ///    let expected = string.to_owned();
    ///
    ///    assert_eq!(String::from_bytes(&bytes).unwrap(), expected);
    /// }
    ///
    /// test_string("", &[0x00], &[0x00, 0x00, 0x00]);
    /// test_string("Hi", &[0x02], &[0x0]);
    /// test_string("Hi!", &[0x03], &[]);
    /// test_string("Hello", &[0x05], &[0x0, 0x0]);
    /// test_string("Hello, world!", &[0xd], &[0x0, 0x0]);
    /// test_string(
    ///     "This is a very long string, and it has to be longer than 253 \
    ///      characters, which are quite a few but we can make it! Although, \
    ///      it is quite challenging. The quick brown fox jumps over the lazy \
    ///      fox. There is still some more text we need to type. Oh, this \
    ///      sentence made it past!",
    ///      &[0xfe, 0x11, 0x01, 0x00],
    ///      &[0x00, 0x00, 0x00]
    /// );
    /// ```
    fn deserialize(buf: Buffer) -> Result<Self> {
        Ok(String::from_utf8_lossy(&Vec::<u8>::deserialize(buf)?).into())
    }
}

impl Deserializable for Vec<u8> {
    /// Deserializes a vector of bytes as a byte-string according to the
    /// following definition:
    ///
    /// * `string ? = String;`.
    ///
    /// # Examples
    ///
    /// ```
    /// use grammers_tl_types::{Deserializable};
    ///
    /// assert_eq!(Vec::<u8>::from_bytes(&[0x00, 0x00, 0x00, 0x00]).unwrap(), Vec::new());
    /// assert_eq!(Vec::<u8>::from_bytes(&[0x01, 0x7f, 0x00, 0x00]).unwrap(), vec![0x7f_u8]);
    /// ```
    fn deserialize(buf: Buffer) -> Result<Self> {
        let first_byte = buf.read_byte()?;
        let (len, padding) = if first_byte == 254 {
            let mut buffer = [0u8; 3];
            buf.read_exact(&mut buffer)?;
            let len =
                (buffer[0] as usize) | ((buffer[1] as usize) << 8) | ((buffer[2] as usize) << 16);

            (len, len % 4)
        } else {
            let len = first_byte as usize;
            (len, (len + 1) % 4)
        };

        let mut result = vec![0u8; len];
        buf.read_exact(&mut result)?;

        if padding > 0 {
            for _ in 0..(4 - padding) {
                buf.read_byte()?;
            }
        }

        Ok(result)
    }
}

#[cfg(test)]
mod tests {
    #[cfg(feature = "tl-api")]
    use super::*;

    #[test]
    #[cfg(feature = "tl-api")]
    fn ensure_generic_reads_blob() {
        let blob = [103, 64, 11, 51, 72];
        assert_eq!(
            <crate::functions::InvokeWithLayer as crate::RemoteCall>::Return::from_bytes(&blob)
                .unwrap()
                .0,
            blob
        );
    }
}
