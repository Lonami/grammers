// Copyright 2020 - developers of the `grammers` project.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// https://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or https://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

//! Errors that can occur during the parsing of [Type Language] definitions.
//!
//! [Type Language]: https://core.telegram.org/mtproto/TL
use std::num::ParseIntError;

/// The error type for the parsing operation of [`Definition`]s.
///
/// [`Definition`]: tl/struct.Definition.html
#[derive(Debug, PartialEq)]
pub enum ParseError {
    /// The definition is empty.
    Empty,

    /// The identifier from this definition is invalid.
    InvalidId(ParseIntError),

    /// One of the parameters from this definition was invalid.
    InvalidParam(ParamParseError),

    /// The name information is missing from the definition.
    MissingName,

    /// The type information is missing from the definition.
    MissingType,

    /// The parser does not know how to parse the definition.
    NotImplemented,

    /// The file contained an unknown separator (such as `---foo---`)
    UnknownSeparator,
}

/// The error type for the parsing operation of [`Parameter`]s.
///
/// [`Parameter`]: tl/struct.Parameter.html
#[derive(Debug, PartialEq)]
pub enum ParamParseError {
    /// The parameter was empty.
    Empty,

    /// The flag specification was invalid.
    InvalidFlag,

    /// The generic argument was invalid.
    InvalidGeneric,

    /// The parameter is actually a generic type definition for later
    /// use, such as `{X:Type}`, but it is not a parameter in itself.
    TypeDef { name: String },

    /// The parameter refers to some unknown definition.
    MissingDef,

    /// The parser does not know how to parse the parameter.
    NotImplemented,
}
