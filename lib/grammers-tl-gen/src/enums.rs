// Copyright 2020 - developers of the `grammers` project.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// https://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or https://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

//! Code to generate Rust's `enum`'s from TL definitions.

use crate::grouper;
use crate::metadata::Metadata;
use crate::rustifier::{rusty_namespaced_type_name, rusty_ty_name, rusty_variant_name};
use grammers_tl_parser::tl::{Definition, Type};
use std::io::{self, Write};

/// Writes an enumeration listing all types such as the following rust code:
///
/// ```ignore
/// pub enum Name {
///     Variant(crate::types::Name),
/// }
/// ```
fn write_enum<W: Write>(
    file: &mut W,
    indent: &str,
    ty: &Type,
    metadata: &Metadata,
) -> io::Result<()> {
    if cfg!(feature = "impl-debug") {
        writeln!(file, "{}#[derive(Debug)]", indent)?;
    }

    writeln!(file, "{}#[derive(PartialEq)]", indent)?;
    writeln!(file, "{}pub enum {} {{", indent, rusty_ty_name(ty))?;
    for d in metadata.defs_with_type(ty) {
        write!(file, "{}    {}(", indent, rusty_variant_name(d))?;

        if metadata.is_recursive_def(d) {
            write!(file, "Box<")?;
        }
        write!(file, "{}", rusty_namespaced_type_name(&d))?;
        if metadata.is_recursive_def(d) {
            write!(file, ">")?;
        }

        writeln!(file, "),")?;
    }
    writeln!(file, "{}}}", indent)?;
    Ok(())
}

/// Defines the `impl Serializable` corresponding to the type definitions:
///
/// ```ignore
/// impl crate::Serializable for Name {
///     fn serialize<B: std::io::Write>(&self, buf: &mut B) -> std::io::Result<()> {
///         use crate::Identifiable;
///         match self {
///             Self::Variant(x) => {
///                 crate::types::Name::CONSTRUCTOR_ID.serialize(buf)?;
///                 x.serialize(buf)
///             },
///         }
///     }
/// }
/// ```
fn write_serializable<W: Write>(
    file: &mut W,
    indent: &str,
    ty: &Type,
    metadata: &Metadata,
) -> io::Result<()> {
    writeln!(
        file,
        "{}impl crate::Serializable for {} {{",
        indent,
        rusty_ty_name(ty)
    )?;
    writeln!(
        file,
        "{}    fn serialize<B: std::io::Write>(&self, buf: &mut B) -> std::io::Result<()> {{",
        indent
    )?;

    writeln!(file, "{}        use crate::Identifiable;", indent)?;
    writeln!(file, "{}        match self {{", indent)?;
    for d in metadata.defs_with_type(ty) {
        writeln!(
            file,
            "{}            Self::{}(x) => {{",
            indent,
            rusty_variant_name(d)
        )?;
        writeln!(
            file,
            "{}                {}::CONSTRUCTOR_ID.serialize(buf)?;",
            indent,
            rusty_namespaced_type_name(&d)
        )?;
        writeln!(file, "{}                x.serialize(buf)", indent)?;
        writeln!(file, "{}            }},", indent)?;
    }
    writeln!(file, "{}        }}", indent)?;
    writeln!(file, "{}    }}", indent)?;
    writeln!(file, "{}}}", indent)?;
    Ok(())
}

/// Defines the `impl Deserializable` corresponding to the type definitions:
///
/// ```ignore
/// impl crate::Deserializable for Name {
///     fn deserialize<B: std::io::Read>(buf: &mut B) -> std::io::Result<Self> {
///         use crate::Identifiable;
///         Ok(match u32::deserialize(buf)? {
///             crate::types::Name::CONSTRUCTOR_ID => Self::Variant(crate::types::Name::deserialize(buf)?),
///             _ => return Err(std::io::Error::new(std::io::ErrorKind::InvalidData, ...)),
///         })
///     }
/// }
/// ```
fn write_deserializable<W: Write>(
    file: &mut W,
    indent: &str,
    ty: &Type,
    metadata: &Metadata,
) -> io::Result<()> {
    writeln!(
        file,
        "{}impl crate::Deserializable for {} {{",
        indent,
        rusty_ty_name(ty)
    )?;
    writeln!(
        file,
        "{}    fn deserialize<B: std::io::Read>(buf: &mut B) -> std::io::Result<Self> {{",
        indent
    )?;
    writeln!(file, "{}        use crate::Identifiable;", indent)?;
    writeln!(file, "{}        let id = u32::deserialize(buf)?;", indent)?;
    writeln!(file, "{}        Ok(match id {{", indent)?;
    for d in metadata.defs_with_type(ty) {
        write!(
            file,
            "{}            {}::CONSTRUCTOR_ID => Self::{}(",
            indent,
            rusty_namespaced_type_name(&d),
            rusty_variant_name(d),
        )?;

        if metadata.is_recursive_def(d) {
            write!(file, "Box::new(")?;
        }
        write!(
            file,
            "{}::deserialize(buf)?",
            rusty_namespaced_type_name(&d)
        )?;
        if metadata.is_recursive_def(d) {
            write!(file, ")")?;
        }
        writeln!(file, "),")?;
    }
    writeln!(
        file,
        "{}            _ => return Err(std::io::Error::new(std::io::ErrorKind::InvalidData, \
         crate::errors::UnexpectedConstructor {{ id }})),",
        indent
    )?;
    writeln!(file, "{}        }})", indent)?;
    writeln!(file, "{}    }}", indent)?;
    writeln!(file, "{}}}", indent)?;
    Ok(())
}

/// Defines the `impl From` corresponding to the definition:
///
/// ```ignore
/// impl impl From<Name> for Enum {
/// }
/// ```
fn write_impl_from<W: Write>(
    file: &mut W,
    indent: &str,
    ty: &Type,
    metadata: &Metadata,
) -> io::Result<()> {
    for def in metadata.defs_with_type(ty) {
        writeln!(
            file,
            "{}impl From<{}> for {} {{",
            indent,
            rusty_namespaced_type_name(def),
            rusty_ty_name(ty),
        )?;
        writeln!(
            file,
            "{}    fn from(x: {}) -> Self {{",
            indent,
            rusty_namespaced_type_name(def),
        )?;
        writeln!(
            file,
            "{}        {cls}::{variant}({box_}x{paren})",
            indent,
            cls = rusty_ty_name(ty),
            box_ = if metadata.is_recursive_def(def) {
                "Box::new("
            } else {
                ""
            },
            variant = rusty_variant_name(def),
            paren = if metadata.is_recursive_def(def) {
                ")"
            } else {
                ""
            },
        )?;
        writeln!(file, "{}    }}", indent)?;
        writeln!(file, "{}}}", indent)?;
    }
    Ok(())
}

/// Writes an entire definition as Rust code (`enum` and `impl`).
fn write_definition<W: Write>(
    file: &mut W,
    indent: &str,
    ty: &Type,
    metadata: &Metadata,
) -> io::Result<()> {
    write_enum(file, indent, ty, metadata)?;
    write_serializable(file, indent, ty, metadata)?;
    write_deserializable(file, indent, ty, metadata)?;
    if cfg!(feature = "impl-from-type") {
        write_impl_from(file, indent, ty, metadata)?;
    }
    Ok(())
}

/// Write the entire module dedicated to enums.
pub(crate) fn write_enums_mod<W: Write>(
    mut file: &mut W,
    definitions: &[Definition],
    metadata: &Metadata,
) -> io::Result<()> {
    // Begin outermost mod
    write!(
        file,
        "\
         /// This module contains all of the boxed types, each\n\
         /// represented by a `enum`. All of them implement\n\
         /// [`Serializable`] and [`Deserializable`].\n\
         ///\n\
         /// [`Serializable`]: /grammers_tl_types/trait.Serializable.html\n\
         /// [`Deserializable`]: /grammers_tl_types/trait.Deserializable.html\n\
         #[allow(clippy::large_enum_variant)]\n\
         pub mod enums {{\n\
         "
    )?;

    let grouped = grouper::group_types_by_ns(definitions);
    let mut sorted_keys: Vec<&Option<String>> = grouped.keys().collect();
    sorted_keys.sort();
    for key in sorted_keys.into_iter() {
        // Begin possibly inner mod
        let indent = if let Some(ns) = key {
            writeln!(file, "    #[allow(clippy::large_enum_variant)]")?;
            writeln!(file, "    pub mod {} {{", ns)?;
            "        "
        } else {
            "    "
        };

        for ty in grouped[key].iter() {
            write_definition(&mut file, indent, ty, metadata)?;
        }

        // End possibly inner mod
        if key.is_some() {
            writeln!(file, "    }}")?;
        }
    }

    // End outermost mod
    writeln!(file, "}}")
}
