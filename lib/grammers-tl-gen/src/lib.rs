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

use grammers_tl_parser::tl::{Category, Definition, Type};
use std::io::{self, Write};

pub struct Config {
    pub gen_name_for_id: bool,
    pub deserializable_functions: bool,
    pub impl_debug: bool,
    pub impl_from_type: bool,
    pub impl_from_enum: bool,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            gen_name_for_id: false,
            deserializable_functions: false,
            impl_debug: true,
            impl_from_type: true,
            impl_from_enum: true,
        }
    }
}

/// Don't generate types for definitions of this type,
/// since they are "core" types and treated differently.
const SPECIAL_CASED_TYPES: [&str; 1] = ["Bool"];

fn ignore_type(ty: &Type) -> bool {
    SPECIAL_CASED_TYPES.iter().any(|&x| x == ty.name)
}

pub fn generate_rust_code(
    file: &mut impl Write,
    definitions: &[Definition],
    layer: i32,
    config: &Config,
) -> io::Result<()> {
    writeln!(
        file,
        r#"
// Copyright 2020 - developers of the `grammers` project.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// https://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or https://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

/// The schema layer from which the definitions were generated.
pub const LAYER: i32 = {};
"#,
        layer
    )?;

    if config.gen_name_for_id {
        writeln!(
            file,
            r#"
/// Return the name from the `.tl` definition corresponding to the provided definition identifier.
pub fn name_for_id(id: u32) -> &'static str {{
    match id {{
        "#
        )?;
        for def in definitions {
            writeln!(file, r#"        0x{:x} => "{}","#, def.id, def.full_name())?;
        }

        writeln!(
            file,
            r#"
        _ => "(unknown)",
    }}
}}
    "#,
        )?;
    }

    let metadata = metadata::Metadata::new(&definitions);
    structs::write_category_mod(file, Category::Types, definitions, &metadata, config)?;
    structs::write_category_mod(file, Category::Functions, definitions, &metadata, config)?;
    enums::write_enums_mod(file, definitions, &metadata, config)?;

    Ok(())
}
