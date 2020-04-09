// Copyright 2020 - developers of the `grammers` project.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// https://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or https://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

//! Code to generate Rust's `struct`'s from TL definitions.

use crate::grouper;
use crate::metadata::Metadata;
use crate::rustifier::{
    rusty_attr_name, rusty_class_name, rusty_namespaced_class_name, rusty_namespaced_type_name,
    rusty_type, rusty_type_name, rusty_type_path, rusty_variant_name,
};
use grammers_tl_parser::tl::{Category, Definition, ParameterType};
use std::io::{self, Write};

/// Defines the `struct` corresponding to the definition:
///
/// ```ignore
/// pub struct Name {
///     pub field: Type,
/// }
/// ```
fn write_struct<W: Write>(
    file: &mut W,
    indent: &str,
    def: &Definition,
    _metadata: &Metadata,
) -> io::Result<()> {
    // Define struct
    if cfg!(feature = "impl-debug") {
        writeln!(file, "{}#[derive(Debug)]", indent)?;
    }

    writeln!(file, "{}#[derive(PartialEq)]", indent)?;
    writeln!(
        file,
        "{}pub struct {} {{",
        indent,
        rusty_class_name(&def.name)
    )?;
    for param in def.params.iter() {
        match param.ty {
            ParameterType::Flags => {
                // Flags are computed on-the-fly, not stored
            }
            ParameterType::Normal { .. } => {
                writeln!(
                    file,
                    "{}    pub {}: {},",
                    indent,
                    rusty_attr_name(param),
                    rusty_type_name(param)
                )?;
            }
        }
    }
    writeln!(file, "{}}}", indent)?;
    Ok(())
}

/// Defines the `impl Identifiable` corresponding to the definition:
///
/// ```ignore
/// impl crate::Identifiable for Name {
///     fn constructor_id() -> u32 { 123 }
/// }
/// ```
fn write_identifiable<W: Write>(
    file: &mut W,
    indent: &str,
    def: &Definition,
    _metadata: &Metadata,
) -> io::Result<()> {
    writeln!(
        file,
        "{}impl crate::Identifiable for {} {{",
        indent,
        rusty_class_name(&def.name)
    )?;
    writeln!(
        file,
        "{}    const CONSTRUCTOR_ID: u32 = {};",
        indent, def.id
    )?;
    writeln!(file, "{}}}", indent)?;
    Ok(())
}

/// Defines the `impl Serializable` corresponding to the definition:
///
/// ```ignore
/// impl crate::Serializable for Name {
///     fn serialize<B: std::io::Write>(&self, buf: &mut B) -> std::io::Result<()> {
///         self.field.serialize(buf)?;
///         Ok(())
///     }
/// }
/// ```
fn write_serializable<W: Write>(
    file: &mut W,
    indent: &str,
    def: &Definition,
    _metadata: &Metadata,
) -> io::Result<()> {
    writeln!(
        file,
        "{}impl crate::Serializable for {} {{",
        indent,
        rusty_class_name(&def.name)
    )?;
    writeln!(
        file,
        "{}    fn serialize<B: std::io::Write>(&self, {}buf: &mut B) -> std::io::Result<()> {{",
        indent,
        if def.category == Category::Types && def.params.is_empty() {
            "_"
        } else {
            ""
        }
    )?;

    match def.category {
        Category::Types => {
            // Bare types should not write their `CONSTRUCTOR_ID`.
        }
        Category::Functions => {
            // Functions should always write their `CONSTRUCTOR_ID`.
            writeln!(file, "{}        use crate::Identifiable;", indent)?;
            writeln!(
                file,
                "{}        Self::CONSTRUCTOR_ID.serialize(buf)?;",
                indent
            )?;
        }
    }

    for param in def.params.iter() {
        write!(file, "{}        ", indent)?;
        match &param.ty {
            ParameterType::Flags => {
                write!(file, "(0u32")?;

                // Compute flags as a single expression
                for p in def.params.iter() {
                    match &p.ty {
                        ParameterType::Normal {
                            ty,
                            flag: Some(flag),
                        } if flag.name == param.name => {
                            // We make sure this `p` uses the flag we're currently
                            // parsing by comparing (`p`'s) `flag.name == param.name`.

                            // OR (if the flag is present) the correct bit index.
                            // Only the special-cased "true" flags are booleans.
                            write!(
                                file,
                                " | if self.{}{} {{ {} }} else {{ 0 }}",
                                rusty_attr_name(p),
                                if ty.name == "true" { "" } else { ".is_some()" },
                                1 << flag.index
                            )?;
                        }
                        _ => {}
                    }
                }

                writeln!(file, ").serialize(buf)?;")?;
            }
            ParameterType::Normal { ty, flag } => {
                // The `true` bare type is a bit special: it's empty so there
                // is not need to serialize it, but it's used enough to deserve
                // a special case and ignore it.
                if ty.name != "true" {
                    if flag.is_some() {
                        writeln!(
                            file,
                            "if let Some(ref x) = self.{} {{ ",
                            rusty_attr_name(param)
                        )?;
                        writeln!(file, "{}            x.serialize(buf)?;", indent)?;
                        writeln!(file, "{}        }}", indent)?;
                    } else {
                        writeln!(file, "self.{}.serialize(buf)?;", rusty_attr_name(param))?;
                    }
                }
            }
        }
    }

    writeln!(file, "{}        Ok(())", indent)?;
    writeln!(file, "{}    }}", indent)?;
    writeln!(file, "{}}}", indent)?;
    Ok(())
}

/// Defines the `impl Deserializable` corresponding to the definition:
///
/// ```ignore
/// impl crate::Deserializable for Name {
///     fn deserialize<B: std::io::Read>(buf: &mut B) -> std::io::Result<Self> {
///         let field = FieldType::deserialize(buf)?;
///         Ok(Name { field })
///     }
/// }
/// ```
fn write_deserializable<W: Write>(
    file: &mut W,
    indent: &str,
    def: &Definition,
    _metadata: &Metadata,
) -> io::Result<()> {
    writeln!(
        file,
        "{}impl crate::Deserializable for {} {{",
        indent,
        rusty_class_name(&def.name)
    )?;
    writeln!(
        file,
        "{}    fn deserialize<B: std::io::Read>({}buf: &mut B) -> std::io::Result<Self> {{",
        indent,
        if def.params.is_empty() { "_" } else { "" }
    )?;

    for (i, param) in def.params.iter().enumerate() {
        write!(file, "{}        ", indent)?;
        match &param.ty {
            ParameterType::Flags => {
                writeln!(
                    file,
                    "let {} = u32::deserialize(buf)?;",
                    rusty_attr_name(param)
                )?;
            }
            ParameterType::Normal { ty, flag } => {
                if ty.name == "true" {
                    let flag = flag
                        .as_ref()
                        .expect("the `true` type must always be used in a flag");
                    writeln!(
                        file,
                        "let {} = ({} & {}) != 0;",
                        rusty_attr_name(param),
                        flag.name,
                        1 << flag.index
                    )?;
                } else {
                    write!(file, "let {} = ", rusty_attr_name(param))?;
                    if let Some(ref flag) = flag {
                        writeln!(file, "if ({} & {}) != 0 {{", flag.name, 1 << flag.index)?;
                        write!(file, "{}            Some(", indent)?;
                    }
                    if ty.generic_ref {
                        // Deserialization of a generic reference requires
                        // parsing *any* constructor, because the length is
                        // not included anywhere. Unfortunately, we do not
                        // have the machinery to do that; we would need a
                        // single `match` with all the possible constructors!.
                        //
                        // But, if the generic is the last parameter, we can
                        // just read the entire remaining thing.
                        //
                        // This will only potentially happen while
                        // deserializing functions anyway.
                        if i == def.params.len() - 1 {
                            writeln!(
                                file,
                                "{{ let mut tmp = Vec::new(); buf.read_to_end(&mut tmp)?; tmp }}"
                            )?;
                        } else {
                            writeln!(
                                file,
                                "unimplemented!(\"cannot read generic params in the middle\")"
                            )?;
                        }
                    } else {
                        write!(file, "{}::deserialize(buf)?", rusty_type_path(param))?;
                    }
                    if flag.is_some() {
                        writeln!(file, ")")?;
                        writeln!(file, "{}        }} else {{", indent)?;
                        writeln!(file, "{}            None", indent)?;
                        write!(file, "{}        }}", indent)?;
                    }
                    writeln!(file, ";")?;
                }
            }
        }
    }

    writeln!(
        file,
        "{}        Ok({} {{",
        indent,
        rusty_class_name(&def.name)
    )?;

    for param in def.params.iter() {
        write!(file, "{}            ", indent)?;
        match &param.ty {
            ParameterType::Flags => {}
            ParameterType::Normal { .. } => {
                writeln!(file, "{},", rusty_attr_name(param))?;
            }
        }
    }
    writeln!(file, "{}        }})", indent)?;
    writeln!(file, "{}    }}", indent)?;
    writeln!(file, "{}}}", indent)?;
    Ok(())
}

/// Defines the `impl RemoteCall` corresponding to the definition:
///
/// ```ignore
/// impl crate::RemoteCall for Name {
///     type Return = Name;
/// }
/// ```
fn write_rpc<W: Write>(
    file: &mut W,
    indent: &str,
    def: &Definition,
    _metadata: &Metadata,
) -> io::Result<()> {
    writeln!(
        file,
        "{}impl crate::RemoteCall for {} {{",
        indent,
        rusty_class_name(&def.name)
    )?;
    writeln!(file, "{}    type Return = {};", indent, rusty_type(&def.ty))?;
    writeln!(file, "{}}}", indent)?;
    Ok(())
}

/// Defines the `impl TryFrom` corresponding to the definition:
///
/// ```ignore
/// impl impl TryFrom<Enum> for Name {
///     type Error = crate::errors::WrongVariant;
/// }
/// ```
fn write_impl_from<W: Write>(
    file: &mut W,
    indent: &str,
    def: &Definition,
    metadata: &Metadata,
) -> io::Result<()> {
    let infallible = metadata.defs_with_type(&def.ty).len() == 1;

    writeln!(
        file,
        "{}impl {}From<{}> for {} {{",
        indent,
        if infallible { "" } else { "Try" },
        rusty_namespaced_class_name(&def.ty),
        rusty_namespaced_type_name(&def),
    )?;
    if !infallible {
        writeln!(
            file,
            "{}    type Error = crate::errors::WrongVariant;",
            indent
        )?;
    }
    writeln!(
        file,
        "{}    fn {try_}from(x: {cls}) -> {result}Self{error} {{",
        indent,
        try_ = if infallible { "" } else { "try_" },
        cls = rusty_namespaced_class_name(&def.ty),
        result = if infallible { "" } else { "Result<" },
        error = if infallible { "" } else { ", Self::Error>" },
    )?;
    writeln!(file, "{}        match x {{", indent)?;
    writeln!(
        file,
        "{}            {cls}::{name}(x) => {ok}{deref}x{paren},",
        indent,
        cls = rusty_namespaced_class_name(&def.ty),
        name = rusty_variant_name(def),
        ok = if infallible { "" } else { "Ok(" },
        deref = if metadata.is_recursive_def(def) {
            "*"
        } else {
            ""
        },
        paren = if infallible { "" } else { ")" },
    )?;
    if !infallible {
        writeln!(
            file,
            "{}            _ => Err(crate::errors::WrongVariant)",
            indent
        )?;
    }
    writeln!(file, "{}        }}", indent)?;
    writeln!(file, "{}    }}", indent)?;
    writeln!(file, "{}}}", indent)?;
    Ok(())
}

/// Writes an entire definition as Rust code (`struct` and `impl`).
fn write_definition<W: Write>(
    file: &mut W,
    indent: &str,
    def: &Definition,
    metadata: &Metadata,
) -> io::Result<()> {
    write_struct(file, indent, def, metadata)?;
    write_identifiable(file, indent, def, metadata)?;
    write_serializable(file, indent, def, metadata)?;
    if def.category == Category::Types || cfg!(feature = "deserializable-functions") {
        write_deserializable(file, indent, def, metadata)?;
    }
    if def.category == Category::Functions {
        write_rpc(file, indent, def, metadata)?;
    }
    if def.category == Category::Types && cfg!(feature = "impl-from-enum") {
        write_impl_from(file, indent, def, metadata)?;
    }
    Ok(())
}

/// Write an entire module for the desired category.
pub(crate) fn write_category_mod<W: Write>(
    mut file: &mut W,
    category: Category,
    definitions: &[Definition],
    metadata: &Metadata,
) -> io::Result<()> {
    // Begin outermost mod
    match category {
        Category::Types => {
            write!(
                file,
                "\
                 /// This module contains all of the bare types, each\n\
                 /// represented by a `struct`. All of them implement\n\
                 /// [`Identifiable`], [`Serializable`] and [`Deserializable`].\n\
                 ///\n\
                 /// [`Identifiable`]: ../trait.Identifiable.html\n\
                 /// [`Serializable`]: ../trait.Serializable.html\n\
                 /// [`Deserializable`]: ../trait.Deserializable.html\n\
                 #[allow(clippy::cognitive_complexity, clippy::identity_op, clippy::unreadable_literal)]\n\
                 pub mod types {{\n\
                 "
            )?;
        }
        Category::Functions => {
            writeln!(
                file,
                "\
            /// This module contains all of the functions, each\n\
            /// represented by a `struct`. All of them implement\n\
            /// [`Identifiable`] and [`Serializable`].\n\
            ///\n\
            /// To find out the type that Telegram will return upon\n\
            /// invoking one of these requests, check out the associated\n\
            /// type in the corresponding [`RemoteCall`] trait impl.\n\
            ///\n\
            /// [`Identifiable`]: ../trait.Identifiable.html\n\
            /// [`Serializable`]: ../trait.Serializable.html\n\
            /// [`RemoteCall`]: trait.RemoteCall.html\n\
            #[allow(clippy::cognitive_complexity, clippy::identity_op, clippy::unreadable_literal)]\n\
            pub mod functions {{
            "
            )?;
        }
    }

    let grouped = grouper::group_by_ns(definitions, category);
    let mut sorted_keys: Vec<&String> = grouped.keys().collect();
    sorted_keys.sort();
    for key in sorted_keys.into_iter() {
        // Begin possibly inner mod
        let indent = if key.is_empty() {
            "    "
        } else {
            writeln!(file, "    #[allow(clippy::unreadable_literal)]")?;
            writeln!(file, "    pub mod {} {{", key)?;
            "        "
        };

        if category == Category::Types && cfg!(feature = "impl-from-enum") {
            // If all of the conversions are infallible this will be unused.
            // Don't bother checking this beforehand, just allow warnings.
            writeln!(file, "{}#[allow(unused_imports)]", indent)?;
            writeln!(file, "{}use std::convert::TryFrom;", indent)?;
        }

        for definition in grouped[key].iter() {
            write_definition(&mut file, indent, definition, metadata)?;
        }

        // End possibly inner mod
        if !key.is_empty() {
            writeln!(file, "    }}")?;
        }
    }

    // End outermost mod
    writeln!(file, "}}")
}
