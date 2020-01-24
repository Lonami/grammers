// TODO `ty` should be parsed further to accomodate for things like
//      `flags.0?Vector<InputDocument>`
#[derive(Debug, PartialEq)]
pub struct Parameter {
    pub name: String,
    pub ty: String,
}

#[derive(Debug, PartialEq)]
pub struct Definition {
    pub name: String,
    pub id: Option<u32>,
    pub params: Vec<Parameter>,
    pub ty: String,
}

#[derive(Debug)]
pub enum ParseError {
    /// The definition is empty
    EmptyDefinition,

    /// The identifier from this definition is malformed
    MalformedId(std::num::ParseIntError),

    /// The name information is missing from the definition
    MissingName,

    /// The type information is missing from the definition
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

#[derive(Debug)]
enum ParamParseError {
    Empty,
    Unimplemented,
}

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
    #[test]
    fn it_works() {
        assert_eq!(2 + 2, 4);
    }
}
