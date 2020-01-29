pub mod errors;
pub mod tl;
mod tl_iterator;
mod utils;

use tl_iterator::TLIterator;

/// Parses a file full of [Type Language] definitions.
///
/// [Type Language]: https://core.telegram.org/mtproto/TL
pub fn parse_tl_file(contents: &str) -> TLIterator {
    TLIterator::new(contents)
}
