// Copyright 2020 - developers of the `grammers` project.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// https://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or https://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

//! This module gathers all the code generation submodules and coordinates
//! them, feeding them the right data.
mod enums;
mod grouper;
mod metadata;
mod rustifier;
mod structs;

use grammers_tl_parser::tl::Category;
use grammers_tl_parser::tl::Definition;
use std::io::{self, Write};

pub fn generate_code(
    file: &mut impl Write,
    definitions: &[Definition],
    layer: i32,
) -> io::Result<()> {
    writeln!(
        file,
        "\
         // Copyright 2020 - developers of the `grammers` project.\n\
         //\n\
         // Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or\n\
         // https://www.apache.org/licenses/LICENSE-2.0> or the MIT license\n\
         // <LICENSE-MIT or https://opensource.org/licenses/MIT>, at your\n\
         // option. This file may not be copied, modified, or distributed\n\
         // except according to those terms.\n\
         \n\
         /// The schema layer from which the definitions were generated.\n\
         pub const LAYER: i32 = {};\n\
         ",
        layer
    )?;

    let metadata = metadata::Metadata::new(&definitions);
    structs::write_category_mod(file, Category::Types, definitions, &metadata)?;
    structs::write_category_mod(file, Category::Functions, definitions, &metadata)?;
    enums::write_enums_mod(file, definitions, &metadata)?;

    Ok(())
}
