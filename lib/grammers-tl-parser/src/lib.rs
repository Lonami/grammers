// TODO `ty` should be parsed further to accomodate for things like
//      `flags.0?Vector<InputDocument>`
use std::num::ParseIntError;

/// A single parameter, with a name and a type.
#[derive(Debug, PartialEq)]
pub struct Parameter {
    /// The name of the parameter.
    pub name: String,

    /// The string representing the type of the parameter.
    /// Note that further parsing is required to use it,
    /// for example in `flags.0?Vector<InputDocument>`.
    pub ty: String,
}

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
    pub ty: String,
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

/// Parses a single parameter such as `foo:bar`.
fn parse_param(param: &str) -> Result<Parameter, ParamParseError> {
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

    Ok(Parameter {
        name: name.into(),
        ty: ty.into(),
    })
}

/// Parses a [Type Language] definition.
///
/// # Examples
///
/// ```
/// use grammers_tl_parser::*;
///
/// let definition = "foo#1 bar:baz = qux";
/// let expected = Definition {
///     name: "foo".into(),
///     id: Some(1),
///     params: vec![
///         Parameter {
///             name: "bar".into(),
///             ty: "baz".into()
///         }
///     ],
///     ty: "qux".into()
/// };
///
/// assert_eq!(parse_tl_definition(definition).unwrap(), expected);
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

    if ty.is_empty() {
        return Err(ParseError::MissingType);
    }

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
        ty: ty.into(),
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
    fn parse_valid_param() {
        assert_eq!(
            parse_param("foo:bar"),
            Ok(Parameter {
                name: "foo".into(),
                ty: "bar".into()
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
        assert_eq!(def.ty, "d");

        let def = parse_tl_definition("a=d").unwrap();
        assert_eq!(def.name, "a");
        assert_eq!(def.id, None);
        assert_eq!(def.params.len(), 0);
        assert_eq!(def.ty, "d");

        let def = parse_tl_definition("a b:c = d").unwrap();
        assert_eq!(def.name, "a");
        assert_eq!(def.id, None);
        assert_eq!(def.params.len(), 1);
        assert_eq!(def.ty, "d");
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
