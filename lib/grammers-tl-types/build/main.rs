//! This module gathers all the code generation submodules and coordinates
//! them, feeding them the right data.
mod enums;
mod grouper;
mod loader;
mod rustifier;
mod structs;

use grammers_tl_parser::tl::Category;
use std::fs::File;
use std::io::{BufWriter, Write};

fn main() -> std::io::Result<()> {
    let definitions = {
        let mut definitions = Vec::new();
        if cfg!(feature = "tl-api") {
            definitions.extend(loader::load_tl("tl/api.tl")?);
        }
        if cfg!(feature = "tl-mtproto") {
            definitions.extend(loader::load_tl("tl/mtproto.tl")?);
        }
        definitions
    };

    let mut file = BufWriter::new(File::create("src/generated.rs")?);

    structs::write_category_mod(&mut file, Category::Types, &definitions)?;
    structs::write_category_mod(&mut file, Category::Functions, &definitions)?;
    enums::write_enums_mod(&mut file, &definitions)?;

    file.flush()?;

    Ok(())
}
