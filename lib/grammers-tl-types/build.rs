// Copyright 2020 - developers of the `grammers` project.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// https://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or https://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.
use grammers_tl_gen::{generate_rust_code, Config};
use grammers_tl_parser::parse_tl_file;
use grammers_tl_parser::tl::Definition;
use std::env;
use std::fs::File;
use std::io::{self, BufRead, BufReader, BufWriter, Read, Write};
use std::path::Path;

/// Load the type language definitions from a certain file.
/// Parse errors will be printed to `stderr`, and only the
/// valid results will be returned.
fn load_tl(file: &str) -> io::Result<Vec<Definition>> {
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
fn find_layer(file: &str) -> io::Result<Option<i32>> {
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

fn main() -> std::io::Result<()> {
    let layer = match find_layer("tl/api.tl")? {
        Some(x) => x,
        None => panic!("no layer information found in api.tl"),
    };

    let definitions = {
        let mut definitions = Vec::new();
        if cfg!(feature = "tl-api") {
            definitions.extend(load_tl("tl/api.tl")?);
        }
        if cfg!(feature = "tl-mtproto") {
            definitions.extend(load_tl("tl/mtproto.tl")?);
        }
        definitions
    };

    let mut file = BufWriter::new(File::create(
        Path::new(&env::var("OUT_DIR").unwrap()).join("generated.rs"),
    )?);

    let config = Config {
        gen_name_for_id: true,
        deserializable_functions: cfg!(feature = "deserializable-functions"),
        impl_debug: cfg!(feature = "impl-debug"),
        impl_from_enum: cfg!(feature = "impl-from-enum"),
        impl_from_type: cfg!(feature = "impl-from-type"),
    };

    generate_rust_code(&mut file, &definitions, layer, &config)?;
    file.flush()?;
    Ok(())
}
