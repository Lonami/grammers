// Copyright 2020 - developers of the `grammers` project.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// https://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or https://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

//! Code to load definitions from files.

use grammers_tl_parser::parse_tl_file;
use grammers_tl_parser::tl::Definition;
use std::fs::File;
use std::io::{self, BufRead, BufReader, Read};

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

/// Find the `// LAYER #` comment, and return its value if it's valid.
pub(crate) fn find_layer(file: &str) -> io::Result<Option<i32>> {
    const LAYER_MARK: &str = "LAYER";

    Ok(BufReader::new(File::open(file)?).lines().find_map(|line| {
        let line = line.unwrap();
        if line.trim().starts_with("//") {
            if let Some(pos) = line.find(LAYER_MARK) {
                if let Ok(layer) = line[pos + LAYER_MARK.len()..].trim().parse() {
                    return Some(layer);
                }
            }
        }

        None
    }))
}
