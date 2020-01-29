use std::str::FromStr;

use crate::errors::ParamParseError;

/// The type of a definition or a parameter.
#[derive(Debug, PartialEq)]
pub struct Type {
    /// The name of the type.
    pub name: String,

    /// Whether this type is bare or boxed.
    pub bare: bool,

    /// Whether the type name refers to a generic definition.
    pub generic_ref: bool,

    /// If the type has a generic argument, which is its type.
    pub generic_arg: Option<Box<Type>>,
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
            (
                &ty[..pos],
                Some(Box::new(Type::from_str(&ty[pos + 1..ty.len() - 1])?)),
            )
        } else {
            (ty, None)
        };

        if ty.is_empty() {
            return Err(ParamParseError::Empty);
        }

        // Safe to unwrap because we just checked is not empty
        let bare = if let Some(pos) = ty.find('.') {
            &ty[pos + 1..]
        } else {
            ty
        }
        .chars()
        .next()
        .unwrap()
        .is_ascii_lowercase();

        Ok(Self {
            name: ty.into(),
            bare,
            generic_ref,
            generic_arg,
        })
    }
}
