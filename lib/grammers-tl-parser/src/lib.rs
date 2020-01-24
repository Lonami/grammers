use std::num::ParseIntError;

/// Data attached to parameters conditional on flags.
#[derive(Debug, PartialEq)]
pub struct Flag {
    /// The name of the field containing the flags.
    pub name: String,

    /// The bit index for the flag inside the flags variable.
    pub index: usize,
}

/// The type of a definition or a parameter.
#[derive(Debug, PartialEq)]
pub struct Type {
    /// The name of the type.
    name: String,

    /// If the type has a generic argument, which one is it.
    generic_arg: Option<String>,
}

/// A parameter type.
#[derive(Debug, PartialEq)]
pub enum ParameterType {
    /// This parameter represents a flags field (`u32`).
    Flags,

    /// A "normal" type, which may depend on a flag.
    Normal {
        /// The actual type of the parameter.
        ty: Type,

        /// If this parameter is conditional, which
        /// flag is used to determine its presence.
        flag: Option<Flag>,
    },
}

/// A single parameter, with a name and a type.
#[derive(Debug, PartialEq)]
pub struct Parameter {
    /// The name of the parameter.
    pub name: String,

    /// The type of the parameter.
    pub ty: ParameterType,
}

// TODO `impl Display`
/// A [Type Language] definition.
///
/// [Type Language]: https://core.telegram.org/mtproto/TL
#[derive(Debug, PartialEq)]
pub struct Definition {
    /// The name of this definition. Also known as "predicate" or "method".
    pub name: String,

    /// The numeric identifier of this definition.
    pub id: Option<u32>,

    /// A possibly-empty list of parameters this definition has.
    pub params: Vec<Parameter>,

    /// The type to which this definition belongs to.
    pub ty: Type,
}

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
}

/// Represents a failure when parsing a single parameter.
#[derive(Debug, PartialEq)]
enum ParamParseError {
    /// The flag was malformed (missing dot, bad index, empty name).
    BadFlag,

    /// The generic argument was malformed (missing closing bracket).
    BadGeneric,

    /// The parameter was empty.
    Empty,

    /// No known way to parse this parameter.
    Unimplemented,
}

/// Removes all single-line comments from the contents.
fn remove_tl_comments(contents: &str) -> String {
    let mut result = String::with_capacity(contents.len());
    let mut in_comment = false;

    contents.chars().enumerate().for_each(|(i, c)| {
        if contents[i..contents.len().min(i + 2)] == *"//" {
            in_comment = true;
        } else if in_comment && c == '\n' {
            in_comment = false;
        }

        if !in_comment {
            result.push(c);
        }
    });

    result.shrink_to_fit();
    result
}

/// Parses a single type `type<generic_arg>`
fn parse_type(ty: &str) -> Result<Type, ParamParseError> {
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
        generic_arg,
    })
}

/// Parses a single parameter such as `foo:bar`.
fn parse_param(param: &str) -> Result<Parameter, ParamParseError> {
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
    let ty = parse_type(ty)?;

    Ok(Parameter {
        name: name.into(),
        ty: ParameterType::Normal { ty, flag },
    })
}

/// Parses a [Type Language] definition.
///
/// # Examples
///
/// ```
/// use grammers_tl_parser::parse_tl_definition;
///
/// assert!(parse_tl_definition("foo#1 bar:baz = qux").is_ok());
///
/// assert!(parse_tl_definition("foo#1 bar:b.0?baz = qux<q>").is_ok());
/// ```
///
/// [Type Language]: https://core.telegram.org/mtproto/TL
pub fn parse_tl_definition(definition: &str) -> Result<Definition, ParseError> {
    if definition.trim().is_empty() {
        return Err(ParseError::EmptyDefinition);
    }

    // Parse `(left = ty)`
    let (left, ty) = {
        let mut it = definition.split('=');
        let ls = it.next().unwrap(); // split() always return at least one
        if let Some(t) = it.next() {
            (ls.trim(), t.trim())
        } else {
            return Err(ParseError::MissingType);
        }
    };

    let ty = parse_type(ty).map_err(|_| ParseError::MissingType)?;

    // Parse `name middle`
    let (name, middle) = {
        if let Some(pos) = left.find(' ') {
            (&left[..pos], left[pos..].trim())
        } else {
            (left.trim(), "")
        }
    };

    // Parse `name#id`
    let (name, id) = {
        let mut it = name.split('#');
        let n = it.next().unwrap(); // split() always return at least one
        (n, it.next())
    };

    if name.is_empty() {
        return Err(ParseError::MissingName);
    }

    // Parse `id`
    let id = match id {
        Some(i) => Some(u32::from_str_radix(i, 16).map_err(ParseError::MalformedId)?),
        None => None,
    };

    // Parse `middle`
    let params = middle
        .split_whitespace()
        .map(|p| {
            parse_param(p).map_err(|e| match e {
                ParamParseError::BadFlag | ParamParseError::BadGeneric => {
                    ParseError::MalformedParam
                }
                ParamParseError::Empty => ParseError::MissingType,
                ParamParseError::Unimplemented => ParseError::NotImplemented {
                    line: definition.trim().into(),
                },
            })
        })
        .collect::<Result<_, ParseError>>()?;

    Ok(Definition {
        name: name.into(),
        id,
        params,
        ty,
    })
}

/// Parses a file full of [Type Language] definitions.
///
/// [Type Language]: https://core.telegram.org/mtproto/TL
pub fn parse_tl_file(contents: &str) -> Vec<Result<Definition, ParseError>> {
    remove_tl_comments(contents)
        .split(';')
        .map(str::trim)
        .filter(|d| !d.is_empty())
        .map(parse_tl_definition)
        .collect()
}

#[cfg(test)]
mod tests {
    use crate::*;

    #[test]
    fn remove_comments_noop() {
        let data = "hello\nworld";
        assert_eq!(remove_tl_comments(data), data);

        let data = " \nhello\nworld\n ";
        assert_eq!(remove_tl_comments(data), data);
    }

    #[test]
    fn remove_comments_leading() {
        let input = " // hello\n world ";
        let expected = " \n world ";
        assert_eq!(remove_tl_comments(input), expected);
    }

    #[test]
    fn remove_comments_trailing() {
        let input = " \nhello \n // world \n \n ";
        let expected = " \nhello \n \n \n ";
        assert_eq!(remove_tl_comments(input), expected);
    }

    #[test]
    fn remove_comments_many() {
        let input = "no\n//yes\nno\n//yes\nno\n";
        let expected = "no\n\nno\n\nno\n";
        assert_eq!(remove_tl_comments(input), expected);
    }

    #[test]
    fn parse_empty_param() {
        assert_eq!(parse_param(":noname"), Err(ParamParseError::Empty));
        assert_eq!(parse_param("notype:"), Err(ParamParseError::Empty));
        assert_eq!(parse_param(":"), Err(ParamParseError::Empty));
    }

    #[test]
    fn parse_unknown_param() {
        assert_eq!(parse_param(""), Err(ParamParseError::Unimplemented));
        assert_eq!(parse_param("no colon"), Err(ParamParseError::Unimplemented));
        assert_eq!(
            parse_param("colonless"),
            Err(ParamParseError::Unimplemented)
        );
    }

    #[test]
    fn parse_bad_flags() {
        assert_eq!(parse_param("foo:bar?"), Err(ParamParseError::BadFlag));
        assert_eq!(parse_param("foo:?bar"), Err(ParamParseError::BadFlag));
        assert_eq!(parse_param("foo:bar?baz"), Err(ParamParseError::BadFlag));
        assert_eq!(parse_param("foo:bar.baz?qux"), Err(ParamParseError::BadFlag));
    }

    #[test]
    fn parse_bad_generics() {
        assert_eq!(parse_param("foo:<bar"), Err(ParamParseError::BadGeneric));
        assert_eq!(parse_param("foo:bar<"), Err(ParamParseError::BadGeneric));
    }

    #[test]
    fn parse_valid_param() {
        assert_eq!(
            parse_param("foo:#"),
            Ok(Parameter {
                name: "foo".into(),
                ty: ParameterType::Flags
            })
        );
        assert_eq!(
            parse_param("foo:bar"),
            Ok(Parameter {
                name: "foo".into(),
                ty: ParameterType::Normal {
                    ty: Type {
                        name: "bar".into(),
                        generic_arg: None,
                    },
                    flag: None,
                }
            })
        );
        assert_eq!(
            parse_param("foo:bar.1?baz"),
            Ok(Parameter {
                name: "foo".into(),
                ty: ParameterType::Normal {
                    ty: Type {
                        name: "baz".into(),
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
            parse_param("foo:bar<baz>"),
            Ok(Parameter {
                name: "foo".into(),
                ty: ParameterType::Normal {
                    ty: Type {
                        name: "bar".into(),
                        generic_arg: Some("baz".into()),
                    },
                    flag: None,
                }
            })
        );
        assert_eq!(
            parse_param("foo:bar.1?baz<qux>"),
            Ok(Parameter {
                name: "foo".into(),
                ty: ParameterType::Normal {
                    ty: Type {
                        name: "baz".into(),
                        generic_arg: Some("qux".into()),
                    },
                    flag: Some(Flag {
                        name: "bar".into(),
                        index: 1,
                    }),
                }
            })
        );
    }

    #[test]
    fn parse_empty_def() {
        assert_eq!(parse_tl_definition(""), Err(ParseError::EmptyDefinition));
    }

    #[test]
    fn parse_bad_id() {
        let bad = u32::from_str_radix("bar", 16).unwrap_err();
        let bad_q = u32::from_str_radix("?", 16).unwrap_err();
        let bad_empty = u32::from_str_radix("", 16).unwrap_err();
        assert_eq!(
            parse_tl_definition("foo#bar = baz"),
            Err(ParseError::MalformedId(bad))
        );
        assert_eq!(
            parse_tl_definition("foo#? = baz"),
            Err(ParseError::MalformedId(bad_q))
        );
        assert_eq!(
            parse_tl_definition("foo# = baz"),
            Err(ParseError::MalformedId(bad_empty))
        );
    }

    #[test]
    fn parse_no_name() {
        assert_eq!(parse_tl_definition(" = foo"), Err(ParseError::MissingName));
    }

    #[test]
    fn parse_no_type() {
        assert_eq!(parse_tl_definition("foo"), Err(ParseError::MissingType));
        assert_eq!(parse_tl_definition("foo = "), Err(ParseError::MissingType));
    }

    #[test]
    fn parse_unimplemented() {
        assert_eq!(
            parse_tl_definition("int ? = Int"),
            Err(ParseError::NotImplemented {
                line: "int ? = Int".into()
            })
        );
    }

    #[test]
    fn parse_valid_definition() {
        let def = parse_tl_definition("a#1=d").unwrap();
        assert_eq!(def.name, "a");
        assert_eq!(def.id, Some(1));
        assert_eq!(def.params.len(), 0);
        assert_eq!(def.ty, Type {
            name: "d".into(),
            generic_arg: None,
        });

        let def = parse_tl_definition("a=d<e>").unwrap();
        assert_eq!(def.name, "a");
        assert_eq!(def.id, None);
        assert_eq!(def.params.len(), 0);
        assert_eq!(def.ty, Type {
            name: "d".into(),
            generic_arg: Some("e".into()),
        });

        let def = parse_tl_definition("a b:c = d").unwrap();
        assert_eq!(def.name, "a");
        assert_eq!(def.id, None);
        assert_eq!(def.params.len(), 1);
        assert_eq!(def.ty, Type {
            name: "d".into(),
            generic_arg: None,
        });
    }

    #[test]
    fn parse_file() {
        let result = &parse_tl_file(
            "
            // leading; comment
            first#1 = t; // inline comment
            second and bad;
            third#3 = t;
            // trailing comment
        ",
        );

        assert_eq!(result.len(), 3);
        assert_eq!(result[0].as_ref().unwrap().id, Some(1));
        assert!(result[1].as_ref().is_err());
        assert_eq!(result[2].as_ref().unwrap().id, Some(3));
    }
}
