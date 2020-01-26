use std::io::{Result, Write};

/// Implementations of this trait will serialize their data into a buffer
/// according to the [Binary Data Serialization].
///
/// [Binary Data Serialization]: https://core.telegram.org/mtproto/serialize
pub trait Serializable {
    /// Serializes the body into the given buffer.
    fn serialize<B: Write>(&self, buf: &mut B) -> Result<()>;

    /// Convenience function to serialize the object and return its bytes.
    fn to_bytes(&self) -> Vec<u8> {
        let mut buffer = Vec::new();
        // Safe to unwrap because `impl Write for Vec<u8>` never fails.
        self.serialize(&mut buffer).unwrap();
        buffer
    }
}

/// Serializes the boolean according to the following definitions:
///
/// * `false` is serialized as `boolFalse#bc799737 = Bool;`.
/// * `true` is serialized as `boolTrue#997275b5 = Bool;`.
///
/// # Examples
///
/// ```
/// use grammers_tl_types::Serializable;
///
/// assert_eq!(true.to_bytes(), [0xb5, 0x75, 0x72, 0x99]);
/// assert_eq!(false.to_bytes(), [0x37, 0x97, 0x79, 0xbc]);
/// ```
impl Serializable for bool {
    fn serialize<B: Write>(&self, buf: &mut B) -> Result<()> {
        if *self { 0x997275b5u32 } else { 0xbc799737u32 }.serialize(buf)
    }
}

/// Serializes the 32-bit signed integer according to the following
/// definition:
///
/// * `int ? = Int;`.
///
/// # Examples
///
/// ```
/// use grammers_tl_types::Serializable;
///
/// assert_eq!(0i32.to_bytes(), [0x00, 0x00, 0x00, 0x00]);
/// assert_eq!(1i32.to_bytes(), [0x01, 0x00, 0x00, 0x00]);
/// assert_eq!((-1i32).to_bytes(), [0xff, 0xff, 0xff, 0xff]);
/// assert_eq!(i32::max_value().to_bytes(), [0xff, 0xff, 0xff, 0x7f]);
/// assert_eq!(i32::min_value().to_bytes(), [0x00, 0x00, 0x00, 0x80]);
/// ```
impl Serializable for i32 {
    fn serialize<B: Write>(&self, buf: &mut B) -> Result<()> {
        buf.write(&self.to_le_bytes()).map(drop)
    }
}

/// Serializes the 32-bit unsigned integer according to the following
/// definition:
///
/// * `int ? = Int;`.
///
/// # Examples
///
/// ```
/// use grammers_tl_types::Serializable;
///
/// assert_eq!(0u32.to_bytes(), [0x00, 0x00, 0x00, 0x00]);
/// assert_eq!(1u32.to_bytes(), [0x01, 0x00, 0x00, 0x00]);
/// assert_eq!(u32::max_value().to_bytes(), [0xff, 0xff, 0xff, 0xff]);
/// assert_eq!(u32::min_value().to_bytes(), [0x00, 0x00, 0x00, 0x00]);
/// ```
impl Serializable for u32 {
    fn serialize<B: Write>(&self, buf: &mut B) -> Result<()> {
        buf.write(&self.to_le_bytes()).map(drop)
    }
}

/// Serializes the 64-bit signed integer according to the following
/// definition:
///
/// * `long ? = Long;`.
///
/// # Examples
///
/// ```
/// use grammers_tl_types::Serializable;
///
/// assert_eq!(0i64.to_bytes(), [0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0]);
/// assert_eq!(1i64.to_bytes(), [0x1, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0]);
/// assert_eq!((-1i64).to_bytes(), [0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff]);
/// assert_eq!(i64::max_value().to_bytes(), [0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0x7f]);
/// assert_eq!(i64::min_value().to_bytes(), [0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x80]);
/// ```
impl Serializable for i64 {
    fn serialize<B: Write>(&self, buf: &mut B) -> Result<()> {
        buf.write(&self.to_le_bytes()).map(drop)
    }
}

/// Serializes the 128-bit integer according to the following definition:
///
/// * `int128 4*[ int ] = Int128;`.
///
/// # Examples
///
/// ```
/// use grammers_tl_types::Serializable;
///
/// let data = [1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16];
///
/// assert_eq!(data.to_bytes(), data);
/// ```
impl Serializable for [u8; 16] {
    fn serialize<B: Write>(&self, buf: &mut B) -> Result<()> {
        buf.write(self).map(drop)
    }
}

/// Serializes the 128-bit integer according to the following definition:
///
/// * `int256 8*[ int ] = Int256;`.
///
/// # Examples
///
/// ```
/// use grammers_tl_types::Serializable;
///
/// let data = [1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17,
///             18, 19, 20, 21, 22, 23, 24, 25, 26, 27, 28, 29, 30, 31, 32];
///
/// assert_eq!(data.to_bytes(), data);
/// ```
impl Serializable for [u8; 32] {
    fn serialize<B: Write>(&self, buf: &mut B) -> Result<()> {
        buf.write(self).map(drop)
    }
}

/// Serializes the 64-bit floating point according to the following
/// definition:
///
/// * `double ? = Double;`.
///
/// # Examples
///
/// ```
/// use std::f64;
/// use grammers_tl_types::Serializable;
///
/// assert_eq!(0f64.to_bytes(), [0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0]);
/// assert_eq!(1.5f64.to_bytes(), [0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0xf8, 0x3f]);
/// assert_eq!((-1.5f64).to_bytes(), [0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0xf8, 0xbf]);
/// assert_eq!(f64::INFINITY.to_bytes(), [0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0xf0, 0x7f]);
/// assert_eq!(f64::NEG_INFINITY.to_bytes(), [0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0xf0, 0xff]);
/// ```
impl Serializable for f64 {
    fn serialize<B: Write>(&self, buf: &mut B) -> Result<()> {
        buf.write(&self.to_le_bytes()).map(drop)
    }
}

/// Serializes a vector of serializable items according to the following
/// definition:
///
/// * `vector#1cb5c415 {t:Type} # [ t ] = Vector t;`.
///
/// # Examples
///
/// ```
/// use grammers_tl_types::Serializable;
///
/// assert_eq!(Vec::<i32>::new().to_bytes(), [0x15, 0xc4, 0xb5, 0x1c, 0x0, 0x0, 0x0, 0x0]);
/// assert_eq!(vec![0x7f_i32].to_bytes(),
///            [0x15, 0xc4, 0xb5, 0x1c, 0x1, 0x0, 0x0, 0x0, 0x7f, 0x0, 0x0, 0x0]);
/// ```
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

/// Serializes a raw vector of serializable items according to the following
/// definition:
///
/// * `vector#1cb5c415 {t:Type} # [ t ] = Vector t;`.
///
/// # Examples
///
/// ```
/// use grammers_tl_types::{RawVec, Serializable};
///
/// assert_eq!(RawVec(Vec::<i32>::new()).to_bytes(), [0x0, 0x0, 0x0, 0x0]);
/// assert_eq!(RawVec(vec![0x7f_i32]).to_bytes(), [0x1, 0x0, 0x0, 0x0, 0x7f, 0x0, 0x0, 0x0]);
/// ```
impl<T: Serializable> Serializable for crate::RawVec<T> {
    fn serialize<B: Write>(&self, buf: &mut B) -> Result<()> {
        (self.0.len() as i32).serialize(buf)?;
        for x in self.0.iter() {
            x.serialize(buf)?;
        }
        Ok(())
    }
}

/// Serializes a UTF-8 string according to the following definition:
///
/// * `string ? = String;`.
///
/// # Examples
///
/// ```
/// use grammers_tl_types::Serializable;
///
/// fn test_string(string: &str, prefix: &[u8], suffix: &[u8]) {
///    let bytes = string.to_owned().to_bytes();
///    let expected = {
///        let mut tmp = prefix.to_vec();
///        tmp.extend(string.as_bytes());
///        tmp.extend(suffix);
///        tmp
///    };
///
///    assert_eq!(bytes, expected);
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
impl Serializable for String {
    fn serialize<B: Write>(&self, buf: &mut B) -> Result<()> {
        self.as_bytes().serialize(buf)
    }
}

/// Serializes a vector of bytes as a byte-string according to the following
/// definition:
///
/// * `string ? = String;`.
///
/// # Examples
///
/// ```
/// use grammers_tl_types::Serializable;
///
/// assert_eq!(Vec::<u8>::new().to_bytes(), &[0x00, 0x00, 0x00, 0x00]);
/// assert_eq!(vec![0x7f_u8].to_bytes(), &[0x01, 0x7f, 0x00, 0x00]);
/// ```
impl Serializable for Vec<u8> {
    fn serialize<B: Write>(&self, buf: &mut B) -> Result<()> {
        (&self[..]).serialize(buf)
    }
}

/// Serializes a byte-string according to the following definition:
///
/// * `string ? = String;`.
///
/// # Examples
///
/// ```
/// use grammers_tl_types::Serializable;
///
/// assert_eq!((&[0x7f_u8][..]).to_bytes(), &[0x01, 0x7f, 0x00, 0x00]);
/// ```
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
