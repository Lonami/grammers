// TODO `ty` should be parsed further to accomodate for things like
//      `flags.0?Vector<InputDocument>`

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
#[derive(Debug)]
pub enum ParseError {
    /// The definition is empty.
    EmptyDefinition,

    /// The identifier from this definition is malformed.
    MalformedId(std::num::ParseIntError),

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
#[derive(Debug)]
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

    contents
        .chars()
        .take(contents.len() - 1)
        .enumerate()
        .for_each(|(i, c)| {
            if contents[i..i + 2] == *"//" {
                in_comment = true;
            } else if in_comment {
                if c == '\n' {
                    in_comment = false;
                }
            } else {
                result.push(c);
            }
        });

    result.shrink_to_fit();
    result
}

/// Parses a single parameter.
///
/// # Examples
///
/// ```
/// assert_eq!(parse_param("foo:int"), Parameter {
///     name: "foo".into(),
///     ty: "int".into(),
/// });
/// ```
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
/// let definition = "foo#1 bar:baz = qux"
/// let expected = Definition {
///     name: "foo".into(),
///     id: Some(1),
///     parameters: vec![
///         Parameter {
///             name: "bar".into(),
///             ty: "baz".into()
///         }
///     ],
///     ty: "qux".into()
/// };
/// assert_eq!(parse_tl_definition(definition), expected);
/// ```
///
/// [Type Language]: https://core.telegram.org/mtproto/TL
pub fn parse_tl_definition(definition: &str) -> Result<Definition, ParseError> {
    // Parse `(left = ty)`
    let (left, ty) = {
        let mut it = definition.split('=');
        if let Some(ls) = it.next() {
            if let Some(t) = it.next() {
                (ls.trim(), t.trim())
            } else {
                dbg!(definition);
                return Err(ParseError::MissingType);
            }
        } else {
            return Err(ParseError::EmptyDefinition);
        }
    };

    // Parse `name middle`
    let (name, middle) = {
        if let Some(pos) = left.find(' ') {
            (&left[..pos], left[pos..].trim())
        } else {
            (left, "")
        }
    };

    // Parse `name#id`
    let (name, id) = {
        let mut it = name.split('#');
        if let Some(n) = it.next() {
            (n, it.next())
        } else {
            return Err(ParseError::MissingName);
        }
    };

    // Parse `id`
    let id = match id {
        Some(i) => Some(u32::from_str_radix(i, 16).map_err(ParseError::MalformedId)?),
        None => None,
    };

    // Parse `middle`
    let params = if middle.is_empty() {
        Vec::new()
    } else {
        middle
            .split(' ')
            .map(|p| {
                parse_param(p).map_err(|e| match e {
                    ParamParseError::Empty => ParseError::MissingType,
                    ParamParseError::Unimplemented => ParseError::NotImplemented {
                        line: definition.trim().into(),
                    },
                })
            })
            .collect::<Result<_, ParseError>>()?
    };

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
