// Copyright 2020 - developers of the `grammers` project.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// https://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or https://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.
use std::fmt;
use std::str::FromStr;

use crate::errors::ParamParseError;

/// The type of a definition or a parameter.
#[derive(Debug, PartialEq)]
pub struct Type {
    /// The namespace components of the type.
    pub namespace: Vec<String>,

    /// The name of the type.
    pub name: String,

    /// Whether this type is bare or boxed.
    pub bare: bool,

    /// Whether the type name refers to a generic definition.
    pub generic_ref: bool,

    /// If the type has a generic argument, which is its type.
    pub generic_arg: Option<Box<Type>>,
}

impl fmt::Display for Type {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for ns in self.namespace.iter() {
            write!(f, "{}.", ns)?;
        }
        if self.generic_ref {
            write!(f, "!")?;
        }
        write!(f, "{}", self.name)?;
        if let Some(generic_arg) = &self.generic_arg {
            write!(f, "<{}>", generic_arg)?;
        }
        Ok(())
    }
}

impl Type {
    /// Find all the nested generic references in this type, and appends them
    /// to the input vector.Box
    pub(crate) fn find_generic_refs<'a>(&'a self, output: &mut Vec<&'a str>) {
        if self.generic_ref {
            output.push(&self.name);
        }
        if let Some(generic_arg) = &self.generic_arg {
            generic_arg.find_generic_refs(output);
        }
    }
}

impl FromStr for Type {
    type Err = ParamParseError;

    /// Parses a type.
    ///
    /// # Examples
    ///
    /// ```
    /// use grammers_tl_parser::tl::Type;
    ///
    /// assert!("vector<int>".parse::<Type>().is_ok());
    /// ```
    fn from_str(ty: &str) -> Result<Self, Self::Err> {
        // Parse `!type`
        let (ty, generic_ref) = if let Some(ty) = ty.strip_prefix('!') {
            (ty, true)
        } else {
            (ty, false)
        };

        // Parse `type<generic_arg>`
        let (ty, generic_arg) = if let Some(pos) = ty.find('<') {
            if !ty.ends_with('>') {
                return Err(ParamParseError::InvalidGeneric);
            }
            (
                &ty[..pos],
                Some(Box::new(Type::from_str(&ty[pos + 1..ty.len() - 1])?)),
            )
        } else {
            (ty, None)
        };

        // Parse `ns1.ns2.name`
        let mut namespace: Vec<String> = ty.split('.').map(|part| part.to_string()).collect();
        if namespace.iter().any(|part| part.is_empty()) {
            return Err(ParamParseError::Empty);
        }

        // Safe to unwrap because split() will always yield at least one.
        let name = namespace.pop().unwrap();

        // Safe to unwrap because we just checked is not empty
        let bare = name.chars().next().unwrap().is_ascii_lowercase();

        Ok(Self {
            namespace,
            name,
            bare,
            generic_ref,
            generic_arg,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn check_empty_simple() {
        assert_eq!(Type::from_str(""), Err(ParamParseError::Empty));
    }

    #[test]
    fn check_simple() {
        assert_eq!(
            Type::from_str("foo"),
            Ok(Type {
                namespace: vec![],
                name: "foo".into(),
                bare: true,
                generic_ref: false,
                generic_arg: None,
            })
        );
    }

    #[test]
    fn check_empty_namespaced() {
        assert_eq!(Type::from_str("."), Err(ParamParseError::Empty));
        assert_eq!(Type::from_str(".."), Err(ParamParseError::Empty));
        assert_eq!(Type::from_str(".foo"), Err(ParamParseError::Empty));
        assert_eq!(Type::from_str("foo."), Err(ParamParseError::Empty));
        assert_eq!(Type::from_str("foo..foo"), Err(ParamParseError::Empty));
        assert_eq!(Type::from_str(".foo."), Err(ParamParseError::Empty));
    }

    #[test]
    fn check_namespaced() {
        assert_eq!(
            Type::from_str("foo.bar.baz"),
            Ok(Type {
                namespace: vec!["foo".into(), "bar".into()],
                name: "baz".into(),
                bare: true,
                generic_ref: false,
                generic_arg: None,
            })
        );
    }

    #[test]
    fn check_bare() {
        assert!(match Type::from_str("foo") {
            Ok(Type { bare: true, .. }) => true,
            _ => false,
        });
        assert!(match Type::from_str("Foo") {
            Ok(Type { bare: false, .. }) => true,
            _ => false,
        });
        assert!(match Type::from_str("Foo.bar") {
            Ok(Type { bare: true, .. }) => true,
            _ => false,
        });
        assert!(match Type::from_str("Foo.Bar") {
            Ok(Type { bare: false, .. }) => true,
            _ => false,
        });
        assert!(match Type::from_str("foo.Bar") {
            Ok(Type { bare: false, .. }) => true,
            _ => false,
        });
        assert!(match Type::from_str("!bar") {
            Ok(Type { bare: true, .. }) => true,
            _ => false,
        });
        assert!(match Type::from_str("!foo.Bar") {
            Ok(Type { bare: false, .. }) => true,
            _ => false,
        });
    }

    #[test]
    fn check_generic_ref() {
        assert!(match Type::from_str("f") {
            Ok(Type {
                generic_ref: false, ..
            }) => true,
            _ => false,
        });
        assert!(match Type::from_str("!f") {
            Ok(Type {
                generic_ref: true, ..
            }) => true,
            _ => false,
        });
        assert!(match Type::from_str("!Foo") {
            Ok(Type {
                generic_ref: true, ..
            }) => true,
            _ => false,
        });
        assert!(match Type::from_str("!X") {
            Ok(Type {
                generic_ref: true, ..
            }) => true,
            _ => false,
        });
    }

    #[test]
    fn check_generic_arg() {
        assert!(match Type::from_str("foo.bar") {
            Ok(Type {
                generic_arg: None, ..
            }) => true,
            _ => false,
        });
        assert!(match Type::from_str("foo<bar>") {
            Ok(Type {
                generic_arg: Some(x),
                ..
            }) => *x == "bar".parse().unwrap(),
            _ => false,
        });
        assert!(match Type::from_str("foo<bar.Baz>") {
            Ok(Type {
                generic_arg: Some(x),
                ..
            }) => *x == "bar.Baz".parse().unwrap(),
            _ => false,
        });
        assert!(match Type::from_str("foo<!bar.baz>") {
            Ok(Type {
                generic_arg: Some(x),
                ..
            }) => *x == "!bar.baz".parse().unwrap(),
            _ => false,
        });
        assert!(match Type::from_str("foo<bar<baz>>") {
            Ok(Type {
                generic_arg: Some(x),
                ..
            }) => *x == "bar<baz>".parse().unwrap(),
            _ => false,
        });
    }
}
