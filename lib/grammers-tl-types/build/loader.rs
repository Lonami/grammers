//! Code to load definitions from files.

use grammers_tl_parser::parse_tl_file;
use grammers_tl_parser::tl::Definition;
use std::fs::File;
use std::io::{self, Read};

/// Load the type language definitions from a certain file.
/// Parse errors will be printed to `stderr`, and only the
/// valid results will be returned.
pub(crate) fn load_tl(file: &str) -> io::Result<Vec<Definition>> {
    let mut file = File::open(file)?;
    let mut contents = String::new();
    file.read_to_string(&mut contents)?;
    Ok(parse_tl_file(&contents)
        .into_iter()
        .filter_map(|d| match d {
            Ok(d) => Some(d),
            Err(e) => {
                eprintln!("TL: parse error: {:?}", e);
                None
            }
        })
        .collect())
}
