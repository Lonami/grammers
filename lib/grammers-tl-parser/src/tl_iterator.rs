// Copyright 2020 - developers of the `grammers` project.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// https://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or https://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.
use crate::errors::ParseError;
use crate::tl::{Category, Definition};
use crate::utils::remove_tl_comments;

const DEFINITION_SEP: &str = ";";
const FUNCTIONS_SEP: &str = "---functions---";
const TYPES_SEP: &str = "---types---";

/// An iterator over [Type Language] definitions.
///
/// [Type Language]: https://core.telegram.org/mtproto/TL
pub struct TlIterator {
    contents: String,
    index: usize,
    category: Category,
}

impl TlIterator {
    pub(crate) fn new(contents: &str) -> Self {
        TlIterator {
            contents: remove_tl_comments(contents),
            index: 0,
            category: Category::Types,
        }
    }
}

impl Iterator for TlIterator {
    type Item = Result<Definition, ParseError>;

    fn next(&mut self) -> Option<Self::Item> {
        let definition = loop {
            if self.index >= self.contents.len() {
                return None;
            }
            let end = if let Some(end) = self.contents[self.index..].find(DEFINITION_SEP) {
                self.index + end
            } else {
                self.contents.len()
            };

            let definition = self.contents[self.index..end].trim();
            self.index = end + DEFINITION_SEP.len();

            if !definition.is_empty() {
                break definition;
            }
        };

        // Get rid of the leading separator and adjust category
        let definition = if definition.starts_with("---") {
            if let Some(definition) = definition.strip_prefix(FUNCTIONS_SEP) {
                self.category = Category::Functions;
                definition.trim()
            } else if let Some(definition) = definition.strip_prefix(TYPES_SEP) {
                self.category = Category::Types;
                definition.trim()
            } else {
                return Some(Err(ParseError::UnknownSeparator));
            }
        } else {
            definition
        };

        // Yield the fixed definition
        Some(match definition.parse::<Definition>() {
            Ok(mut d) => {
                d.category = self.category;
                Ok(d)
            }
            x => x,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::errors::ParseError;

    #[test]
    fn parse_bad_separator() {
        let mut it = TlIterator::new("---foo---");
        assert_eq!(it.next(), Some(Err(ParseError::UnknownSeparator)));
        assert_eq!(it.next(), None);
    }

    #[test]
    fn parse_file() {
        let mut it = TlIterator::new(
            "
            // leading; comment
            first#1 = t; // inline comment
            second and bad;
            third#3 = t;
            // trailing comment
        ",
        );

        assert_eq!(it.next().unwrap().unwrap().id, 1);
        assert!(it.next().unwrap().is_err());
        assert_eq!(it.next().unwrap().unwrap().id, 3);
        assert_eq!(it.next(), None);
    }
}
