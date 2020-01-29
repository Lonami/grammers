//! This module contains the errors that can occur during the parsing
//! of [Type Language] definitions.
///
/// [Type Language]: https://core.telegram.org/mtproto/TL

use std::num::ParseIntError;

/// Represents a failure when parsing [Type Language] definitions.
///
/// [Type Language]: https://core.telegram.org/mtproto/TL
#[derive(Debug, PartialEq)]
pub enum ParseError {
    /// The definition is empty.
    EmptyDefinition,

    /// The identifier from this definition is malformed.
    MalformedId(ParseIntError),

    /// Some parameter of this definition is malformed.
    MalformedParam,

    /// The name information is missing from the definition.
    MissingName,

    /// The type information is missing from the definition.
    MissingType,

    /// The parser does not know how to parse the definition.
    ///
    /// Some unimplemented examples are:
    ///
    /// ```text
    /// int ? = Int;
    /// vector {t:Type} # [ t ] = Vector t;
    /// int128 4*[ int ] = Int128;
    /// ```
    NotImplemented { line: String },

    /// The file contained an unknown separator (such as `---foo---`)
    UnknownSeparator,
}

/// Represents a failure when parsing a single parameter.
#[derive(Debug, PartialEq)]
pub enum ParamParseError {
    /// The flag was malformed (missing dot, bad index, empty name).
    BadFlag,

    /// The generic argument was malformed (missing closing bracket).
    BadGeneric,

    /// The parameter was empty.
    Empty,

    /// The parameter is actually a generic type definition for later use,
    /// such as `{X:Type}`.
    TypeDef { name: String },

    /// Similar to `TypeDef`, but we don't know what it defines.
    UnknownDef,

    /// No known way to parse this parameter.
    Unimplemented,
}
