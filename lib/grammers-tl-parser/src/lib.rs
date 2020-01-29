pub mod tl;
pub mod errors;
mod utils;

use crate::tl::{Category, Definition};
use crate::errors::ParseError;
use crate::utils::remove_tl_comments;

const FUNCTIONS_SEP: &'static str = "---functions---";
const TYPES_SEP: &'static str = "---types---";

/// Parses a file full of [Type Language] definitions.
///
/// [Type Language]: https://core.telegram.org/mtproto/TL
pub fn parse_tl_file(contents: &str) -> Vec<Result<Definition, ParseError>> {
    let mut category = Category::Types;
    let mut result = Vec::new();

    remove_tl_comments(contents)
        .split(';')
        .map(str::trim)
        .filter(|d| !d.is_empty())
        .for_each(|d| {
            // Get rid of the leading separator and adjust category
            let d = if d.starts_with("---") {
                if d.starts_with(FUNCTIONS_SEP) {
                    category = Category::Functions;
                    d[FUNCTIONS_SEP.len()..].trim()
                } else if d.starts_with(TYPES_SEP) {
                    category = Category::Types;
                    d[TYPES_SEP.len()..].trim()
                } else {
                    result.push(Err(ParseError::UnknownSeparator));
                    return;
                }
            } else {
                d
            };

            // Save the fixed definition
            result.push(match d.parse::<Definition>() {
                Ok(mut d) => {
                    d.category = category;
                    Ok(d)
                }
                x => x,
            });
        });

    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_bad_separator() {
        let result = parse_tl_file("---foo---");
        assert_eq!(result.len(), 1);

        match &result[0] {
            Ok(_) => panic!("result should be err"),
            Err(e) => {
                assert_eq!(*e, ParseError::UnknownSeparator);
            }
        }
    }

    #[test]
    fn parse_file() {
        let result = parse_tl_file(
            "
            // leading; comment
            first#1 = t; // inline comment
            second and bad;
            third#3 = t;
            // trailing comment
        ",
        );

        assert_eq!(result.len(), 3);
        assert_eq!(result[0].as_ref().unwrap().id, 1);
        assert!(result[1].as_ref().is_err());
        assert_eq!(result[2].as_ref().unwrap().id, 3);
    }
}
