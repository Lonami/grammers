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
use crate::tl::ParameterType;

/// A single parameter, with a name and a type.
#[derive(Debug, PartialEq, Eq, Hash)]
pub struct Parameter {
    /// The name of the parameter.
    pub name: String,

    /// The type of the parameter.
    pub ty: ParameterType,
}

impl fmt::Display for Parameter {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}:{}", self.name, self.ty)
    }
}

impl FromStr for Parameter {
    type Err = ParamParseError;

    /// Parses a parameter.
    ///
    /// # Examples
    ///
    /// ```
    /// use grammers_tl_parser::tl::Parameter;
    ///
    /// assert!("foo:flags.0?bar.Baz".parse::<Parameter>().is_ok());
    /// ```
    fn from_str(param: &str) -> Result<Self, Self::Err> {
        // Special case: parse `{X:Type}`
        if let Some(def) = param.strip_prefix('{') {
            return Err(if let Some(def) = def.strip_suffix(":Type}") {
                ParamParseError::TypeDef { name: def.into() }
            } else {
                ParamParseError::MissingDef
            });
        };

        // Parse `name:type`
        let (name, ty) = match param.split_once(':') {
            Some((name, ty)) => (name, ty),
            None => return Err(ParamParseError::NotImplemented),
        };
        if name.is_empty() || ty.is_empty() {
            return Err(ParamParseError::Empty);
        }

        Ok(Parameter {
            name: name.into(),
            ty: ty.parse()?,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tl::{Flag, Type};

    #[test]
    fn parse_empty_param() {
        for param_str in [":noname", "notype:", ":"] {
            assert_eq!(
                Parameter::from_str(param_str),
                Err(ParamParseError::Empty),
                "Parameter::from_str({param_str:?})"
            );
        }
    }

    #[test]
    fn parse_unknown_param() {
        for param_str in ["", "no colon", "colonless"] {
            assert_eq!(
                Parameter::from_str(param_str),
                Err(ParamParseError::NotImplemented),
                "Parameter::from_str({param_str:?})"
            );
        }
    }

    #[test]
    fn parse_bad_flags() {
        for param_str in ["foo:bar?", "foo:?bar?", "foo:bar?baz", "foo:bar.baz?qux"] {
            assert_eq!(
                Parameter::from_str(param_str),
                Err(ParamParseError::InvalidFlag),
                "Parameter::from_str({param_str:?})"
            );
        }
    }

    #[test]
    fn parse_bad_generics() {
        assert_eq!(
            Parameter::from_str("foo:<bar"),
            Err(ParamParseError::InvalidGeneric)
        );
        assert_eq!(
            Parameter::from_str("foo:bar<"),
            Err(ParamParseError::InvalidGeneric)
        );
    }

    #[test]
    fn parse_type_def_param() {
        assert_eq!(
            Parameter::from_str("{a:Type}"),
            Err(ParamParseError::TypeDef { name: "a".into() })
        );
    }

    #[test]
    fn parse_unknown_def_param() {
        assert_eq!(
            Parameter::from_str("{a:foo}"),
            Err(ParamParseError::MissingDef)
        );
    }

    #[test]
    fn parse_valid_param() {
        assert_eq!(
            Parameter::from_str("foo:#"),
            Ok(Parameter {
                name: "foo".into(),
                ty: ParameterType::Flags
            })
        );
        assert_eq!(
            Parameter::from_str("foo:!bar"),
            Ok(Parameter {
                name: "foo".into(),
                ty: ParameterType::Normal {
                    ty: Type {
                        namespace: vec![],
                        name: "bar".into(),
                        bare: true,
                        generic_ref: true,
                        generic_arg: None,
                    },
                    flag: None,
                }
            })
        );
        assert_eq!(
            Parameter::from_str("foo:bar.1?baz"),
            Ok(Parameter {
                name: "foo".into(),
                ty: ParameterType::Normal {
                    ty: Type {
                        namespace: vec![],
                        name: "baz".into(),
                        bare: true,
                        generic_ref: false,
                        generic_arg: None,
                    },
                    flag: Some(Flag {
                        name: "bar".into(),
                        index: 1,
                    }),
                }
            })
        );
        assert_eq!(
            Parameter::from_str("foo:bar<baz>"),
            Ok(Parameter {
                name: "foo".into(),
                ty: ParameterType::Normal {
                    ty: Type {
                        namespace: vec![],
                        name: "bar".into(),
                        bare: true,
                        generic_ref: false,
                        generic_arg: Some(Box::new("baz".parse().unwrap())),
                    },
                    flag: None,
                }
            })
        );
        assert_eq!(
            Parameter::from_str("foo:bar.1?baz<qux>"),
            Ok(Parameter {
                name: "foo".into(),
                ty: ParameterType::Normal {
                    ty: Type {
                        namespace: vec![],
                        name: "baz".into(),
                        bare: true,
                        generic_ref: false,
                        generic_arg: Some(Box::new("qux".parse().unwrap())),
                    },
                    flag: Some(Flag {
                        name: "bar".into(),
                        index: 1,
                    }),
                }
            })
        );
    }
}
