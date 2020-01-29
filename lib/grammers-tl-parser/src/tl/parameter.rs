use std::str::FromStr;

use crate::errors::ParamParseError;
use crate::tl::{Flag, ParameterType};

/// A single parameter, with a name and a type.
#[derive(Debug, PartialEq)]
pub struct Parameter {
    /// The name of the parameter.
    pub name: String,

    /// The type of the parameter.
    pub ty: ParameterType,
}

impl FromStr for Parameter {
    type Err = ParamParseError;

    /// Parses a single parameter such as `foo:bar`.
    fn from_str(param: &str) -> Result<Self, Self::Err> {
        // Parse `{X:Type}`
        if param.starts_with('{') {
            return Err(if param.ends_with(":Type}") {
                ParamParseError::TypeDef {
                    // Safe to unwrap because we know it contains ':'
                    name: param[1..param.find(':').unwrap()].into(),
                }
            } else {
                ParamParseError::UnknownDef
            });
        };

        // Parse `name:type`
        let (name, ty) = {
            let mut it = param.split(':');
            if let Some(n) = it.next() {
                if let Some(t) = it.next() {
                    (n, t)
                } else {
                    return Err(ParamParseError::Unimplemented);
                }
            } else {
                return Err(ParamParseError::Empty);
            }
        };

        if name.is_empty() || ty.is_empty() {
            return Err(ParamParseError::Empty);
        }

        // Special-case flags type `#`
        if ty == "#" {
            return Ok(Parameter {
                name: name.into(),
                ty: ParameterType::Flags,
            });
        }

        // Parse `flag_name.flag_index?type`
        let (ty, flag) = if let Some(pos) = ty.find('?') {
            if let Some(dot_pos) = ty.find('.') {
                (
                    &ty[pos + 1..],
                    Some(Flag {
                        name: ty[..dot_pos].into(),
                        index: ty[dot_pos + 1..pos]
                            .parse()
                            .map_err(|_| ParamParseError::BadFlag)?,
                    }),
                )
            } else {
                return Err(ParamParseError::BadFlag);
            }
        } else {
            (ty, None)
        };

        // Parse `type<generic_arg>`
        let ty = ty.parse()?;

        Ok(Parameter {
            name: name.into(),
            ty: ParameterType::Normal { ty, flag },
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tl::Type;

    #[test]
    fn parse_empty_param() {
        assert_eq!(Parameter::from_str(":noname"), Err(ParamParseError::Empty));
        assert_eq!(Parameter::from_str("notype:"), Err(ParamParseError::Empty));
        assert_eq!(Parameter::from_str(":"), Err(ParamParseError::Empty));
    }

    #[test]
    fn parse_unknown_param() {
        assert_eq!(Parameter::from_str(""), Err(ParamParseError::Unimplemented));
        assert_eq!(
            Parameter::from_str("no colon"),
            Err(ParamParseError::Unimplemented)
        );
        assert_eq!(
            Parameter::from_str("colonless"),
            Err(ParamParseError::Unimplemented)
        );
    }

    #[test]
    fn parse_bad_flags() {
        assert_eq!(
            Parameter::from_str("foo:bar?"),
            Err(ParamParseError::BadFlag)
        );
        assert_eq!(
            Parameter::from_str("foo:?bar"),
            Err(ParamParseError::BadFlag)
        );
        assert_eq!(
            Parameter::from_str("foo:bar?baz"),
            Err(ParamParseError::BadFlag)
        );
        assert_eq!(
            Parameter::from_str("foo:bar.baz?qux"),
            Err(ParamParseError::BadFlag)
        );
    }

    #[test]
    fn parse_bad_generics() {
        assert_eq!(
            Parameter::from_str("foo:<bar"),
            Err(ParamParseError::BadGeneric)
        );
        assert_eq!(
            Parameter::from_str("foo:bar<"),
            Err(ParamParseError::BadGeneric)
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
            Err(ParamParseError::UnknownDef)
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
