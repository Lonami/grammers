use std::str::FromStr;

use crate::errors::ParamParseError;

/// The type of a definition or a parameter.
#[derive(Debug, PartialEq)]
pub struct Type {
    /// The name of the type.
    pub name: String,

    /// Whether the type name refers to a generic definition.
    pub generic_ref: bool,

    /// If the type has a generic argument, which one is it.
    pub generic_arg: Option<String>,
}

impl FromStr for Type {
    type Err = ParamParseError;

    /// Parses a single type `type<generic_arg>`
    fn from_str(ty: &str) -> Result<Self, Self::Err> {
        // Parse `!type`
        let (ty, generic_ref) = if ty.starts_with('!') {
            (&ty[1..], true)
        } else {
            (ty, false)
        };

        // Parse `type<generic_arg>`
        let (ty, generic_arg) = if let Some(pos) = ty.find('<') {
            if !ty.ends_with('>') {
                return Err(ParamParseError::BadGeneric);
            }
            (&ty[..pos], Some(ty[pos + 1..ty.len() - 1].into()))
        } else {
            (ty, None)
        };

        if ty.is_empty() {
            return Err(ParamParseError::Empty);
        }

        Ok(Type {
            name: ty.into(),
            generic_ref,
            generic_arg,
        })
    }
}
