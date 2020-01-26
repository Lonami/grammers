//! This module contains several errors that can occur when serializing
//! or deserializing the types.
use std::error::Error;
use std::fmt;

/// This error occurs when an unexpected constructor is found,
/// for example, when reading data that doesn't represent the
/// correct type (e.g. reading a `bool` when we expect a `Vec`).
///
/// * When reading a boolean.
/// * When reading a boxed vector
/// * When reading an arbitrary boxed type.
///
/// It is important to note that unboxed or bare types lack the
/// constructor information, and as such they cannot be validated.
#[derive(Debug)]
pub struct UnexpectedConstructor {
    pub id: u32,
}

impl Error for UnexpectedConstructor {}

impl fmt::Display for UnexpectedConstructor {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "unexpected constructor: {:08x}", self.id)
    }
}
