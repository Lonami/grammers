// Copyright 2020 - developers of the `grammers` project.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// https://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or https://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

//! This library is intended to be a build-time dependency,
//! used to generate source code from parsed TL definitions.

#![deny(unsafe_code)]

mod enums;
mod grouper;
mod metadata;
mod rustifier;
mod structs;

use std::io::{self, Write};

use grammers_tl_parser::tl::{Category, Definition, Type};

/// Writers to use as output for each generated module.
pub struct Outputs<W: Write> {
    /// Writer to the file containing the generated layer constant and name mapping if enabled.
    pub common: W,
    /// Writer to the file containing all of the concrete [`Category::Types`] constructors.
    pub types: W,
    /// Writer to the file containing all of the [`Category::Functions`] constructors.
    pub functions: W,
    /// Writer to the file containing all of the boxed [`Category::Types`].
    pub enums: W,
}

impl<W: Write> Outputs<W> {
    /// Flush all writers sequentially.
    pub fn flush(&mut self) -> std::io::Result<()> {
        self.common.flush()?;
        self.types.flush()?;
        self.functions.flush()?;
        self.enums.flush()
    }
}

/// Configuration used by [`generate_rust_code`].
pub struct Config {
    /// Whether to generate a giant function that will map the constructor ID to its TL name. Useful for debugging.
    pub gen_name_for_id: bool,
    /// Whether to also `impl Deserializable` on the definitions under [`Category::Functions`].
    pub deserializable_functions: bool,
    /// Whether to derive `Debug` for all generated types.
    pub impl_debug: bool,
    /// Whether to `impl From<types::*> for enums::*` for all generated types.
    pub impl_from_type: bool,
    /// Whether to `impl TryFrom<enums::*> for types::*` for all generated types.
    pub impl_from_enum: bool,
    /// Whether to derive `serde::*` for all generated types.
    pub impl_serde: bool,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            gen_name_for_id: false,
            deserializable_functions: false,
            impl_debug: true,
            impl_from_type: true,
            impl_from_enum: true,
            impl_serde: false,
        }
    }
}

/// Don't generate types for definitions of this type,
/// since they are "core" types and treated differently.
const SPECIAL_CASED_TYPES: [&str; 1] = ["Bool"];

fn ignore_type(ty: &Type) -> bool {
    SPECIAL_CASED_TYPES.iter().any(|&x| x == ty.name)
}

/// Generate the Rust code into the provided outputs for the given parsed definitions.
pub fn generate_rust_code<W: Write>(
    outputs: &mut Outputs<W>,
    definitions: &[Definition],
    layer: i32,
    config: &Config,
) -> io::Result<()> {
    writeln!(
        &mut outputs.common,
        r#"/// The schema layer from which the definitions were generated.
pub const LAYER: i32 = {layer};
"#
    )?;

    if config.gen_name_for_id {
        writeln!(
            outputs.common,
            r#"
/// Return the name from the `.tl` definition corresponding to the provided definition identifier.
pub fn name_for_id(id: u32) -> &'static str {{
    match id {{
        0x1cb5c415 => "vector","#
        )?;
        for def in definitions {
            writeln!(
                &mut outputs.common,
                r#"        0x{:x} => "{}","#,
                def.id,
                def.full_name()
            )?;
        }

        writeln!(
            outputs.common,
            r#"
        _ => "(unknown)",
    }}
}}
    "#,
        )?;
    }

    let metadata = metadata::Metadata::new(definitions);
    structs::write_category_mod(
        &mut outputs.types,
        Category::Types,
        definitions,
        &metadata,
        config,
    )?;
    structs::write_category_mod(
        &mut outputs.functions,
        Category::Functions,
        definitions,
        &metadata,
        config,
    )?;
    enums::write_enums_mod(&mut outputs.enums, definitions, &metadata, config)?;

    Ok(())
}
