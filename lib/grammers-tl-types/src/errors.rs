// Copyright 2020 - developers of the `grammers` project.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// https://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or https://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

//! Errors that can occur when using the [`Serializable`] and
//! [`Deserializable`] trait on [`types`] or [`enums`].
//!
//! [`Serializable`]: trait.Serializable.html
//! [`Deserializable`]: trait.Serializable.html
//! [`types`]: types/index.html
//! [`enums`]: enums/index.html
use std::error::Error;
use std::fmt;

#[derive(Debug, Clone, Copy)]
pub enum DeserializeError {
    /// The end of the buffer was reached earlier than anticipated, which
    /// implies there is not enough data to complete the deserialization.
    UnexpectedEof,

    /// The error type indicating an unexpected constructor was found,
    /// for example, when reading data that doesn't represent the
    /// correct type (e.g. reading a `bool` when we expect a `Vec`).
    /// In particular, it can occur in the following situations:
    ///
    /// * When reading a boolean.
    /// * When reading a boxed vector.
    /// * When reading an arbitrary boxed type.
    ///
    /// It is important to note that unboxed or bare [`types`] lack the
    /// constructor information, and as such they cannot be validated.
    ///
    /// [`types`]: types/index.html
    UnexpectedConstructor {
        /// The unexpected constructor identifier.
        id: u32,
    },
}

impl Error for DeserializeError {}

impl fmt::Display for DeserializeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match *self {
            Self::UnexpectedEof => write!(f, "unexpected eof"),
            Self::UnexpectedConstructor { id } => write!(f, "unexpected constructor: {:08x}", id),
        }
    }
}

/// The error type indicating the enumeration is representing a different
/// variant (which is "wrong") and cannot be converted into the desired type.
#[derive(Debug)]
#[cfg(feature = "impl-from-enum")]
pub struct WrongVariant;

#[cfg(feature = "impl-from-enum")]
impl Error for WrongVariant {}

#[cfg(feature = "impl-from-enum")]
impl fmt::Display for WrongVariant {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "enum has a different variant than the requested")
    }
}
