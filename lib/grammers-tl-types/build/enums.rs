//! Code to generate Rust's `enum`'s from TL definitions.

use crate::grouper;
use crate::rustifier::{rusty_class_name, rusty_namespaced_class_name};
use grammers_tl_parser::{Category, Definition, ParameterType};
use std::io::{self, Write};

/// Writes an enumeration listing all types such as the following rust code:
///
/// ```
/// pub enum Name {
///     Variant(crate::types::Name),
/// }
/// ```
fn write_enum<W: Write>(
    file: &mut W,
    indent: &str,
    name: &str,
    type_defs: &Vec<&Definition>,
) -> io::Result<()> {
    writeln!(file, "{}pub enum {} {{", indent, rusty_class_name(name))?;
    for d in type_defs.iter() {
        write!(file, "{}    {}(", indent, rusty_class_name(&d.name))?;

        // Check if this type immediately recurses. If it does, box it.
        // There are no types with indirect recursion, so this works well.
        let recurses = d.params.iter().any(|p| match &p.ty {
            ParameterType::Flags => false,
            ParameterType::Normal { ty, .. } => ty.name == name,
        });

        if recurses {
            write!(file, "Box<")?;
        }
        write!(
            file,
            "crate::types::{}",
            rusty_namespaced_class_name(&d.name)
        )?;
        if recurses {
            write!(file, ">")?;
        }

        writeln!(file, "),")?;
    }
    writeln!(file, "{}}}", indent)?;
    Ok(())
}

/// Defines the `impl Serializable` corresponding to the type definitions:
///
/// ```
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
    name: &str,
    type_defs: &Vec<&Definition>,
) -> io::Result<()> {
    writeln!(
        file,
        "{}impl crate::Serializable for {} {{",
        indent,
        rusty_class_name(name)
    )?;
    writeln!(
        file,
        "{}    fn serialize<B: std::io::Write>(&self, buf: &mut B) -> std::io::Result<()> {{",
        indent
    )?;

    if type_defs.is_empty() {
        writeln!(file, "{}        Ok(())", indent)?;
    } else {
        writeln!(file, "{}        use crate::Identifiable;", indent)?;
        writeln!(file, "{}        match self {{", indent)?;
        for d in type_defs.iter() {
            writeln!(
                file,
                "{}            Self::{}(x) => {{",
                indent,
                rusty_class_name(&d.name)
            )?;
            writeln!(
                file,
                "{}                crate::types::{}::CONSTRUCTOR_ID.serialize(buf)?;",
                indent,
                rusty_namespaced_class_name(&d.name)
            )?;
            writeln!(file, "{}                x.serialize(buf)", indent)?;
            writeln!(file, "{}            }},", indent)?;
        }
        writeln!(file, "{}        }}", indent)?;
    }
    writeln!(file, "{}    }}", indent)?;
    writeln!(file, "{}}}", indent)?;
    Ok(())
}

/// Defines the `impl Deserializable` corresponding to the type definitions:
///
/// ```
/// impl crate::Deserializable for Name {
///     fn deserialize<B: std::io::Read>(buf: &mut B) -> std::io::Result<Self> {
///         use crate::Identifiable;
///         Ok(match u32::deserialize(buf)? {
///             crate::types::Name::CONSTRUCTOR_ID => Self::Variant(crate::types::Name::deserialize(buf)?),
///             _ => unimplemented!("return error")
///         })
///     }
/// }
/// ```
fn write_deserializable<W: Write>(
    file: &mut W,
    indent: &str,
    name: &str,
    type_defs: &Vec<&Definition>,
) -> io::Result<()> {
    writeln!(
        file,
        "{}impl crate::Deserializable for {} {{",
        indent,
        rusty_class_name(name)
    )?;
    writeln!(
        file,
        "{}    fn deserialize<B: std::io::Read>(buf: &mut B) -> std::io::Result<Self> {{",
        indent
    )?;
    writeln!(file, "{}        use crate::Identifiable;", indent)?;
    writeln!(file, "{}        Ok(match u32::deserialize(buf)? {{", indent)?;
    for d in type_defs.iter() {
        write!(
            file,
            "{}            crate::types::{}::CONSTRUCTOR_ID => Self::{}(",
            indent,
            rusty_namespaced_class_name(&d.name),
            rusty_class_name(&d.name),
        )?;

        // TODO this is somewhat expensive (and we're doing it twice)
        let recurses = d.params.iter().any(|p| match &p.ty {
            ParameterType::Flags => false,
            ParameterType::Normal { ty, .. } => ty.name == name,
        });

        if recurses {
            write!(file, "Box::new(")?;
        }
        write!(
            file,
            "crate::types::{}::deserialize(buf)?",
            rusty_namespaced_class_name(&d.name)
        )?;
        if recurses {
            write!(file, ")")?;
        }
        writeln!(file, "),")?;
    }
    writeln!(
        file,
        "{}            _ => unimplemented!(\"return error\")",
        indent
    )?;
    writeln!(file, "{}        }})", indent)?;
    writeln!(file, "{}    }}", indent)?;
    writeln!(file, "{}}}", indent)?;
    Ok(())
}

/// Writes an entire definition as Rust code (`enum` and `impl`).
fn write_definition<W: Write>(
    file: &mut W,
    indent: &str,
    name: &str,
    type_defs: &Vec<&Definition>,
) -> io::Result<()> {
    write_enum(file, indent, name, type_defs)?;
    write_serializable(file, indent, name, type_defs)?;
    write_deserializable(file, indent, name, type_defs)?;
    Ok(())
}

/// Write the entire module dedicated to enums.
pub(crate) fn write_enums_mod<W: Write>(
    mut file: &mut W,
    definitions: &Vec<Definition>,
) -> io::Result<()> {
    // Begin outermost mod
    writeln!(
        file,
        "/// This module contains all of the boxed types, each"
    )?;
    writeln!(file, "/// represented by a `enum`. All of them are")?;
    writeln!(file, "/// `Serializable`.")?;
    writeln!(file, "pub mod enums {{")?;

    let grouped = grouper::group_types_by_ns(definitions);
    let mut sorted_keys: Vec<&String> = grouped.keys().collect();
    sorted_keys.sort();
    for key in sorted_keys.into_iter() {
        // Begin possibly inner mod
        let indent = if key.is_empty() {
            "    "
        } else {
            writeln!(file, "    pub mod {} {{", key)?;
            "        "
        };

        for name in grouped[key].iter() {
            let type_defs: Vec<&Definition> = definitions
                .into_iter()
                .filter(|d| d.category == Category::Types && d.ty.name == **name)
                .collect();

            assert!(!type_defs.is_empty(), "type defs should not be empty");
            write_definition(&mut file, indent, name, &type_defs)?;
        }

        // End possibly inner mod
        if !key.is_empty() {
            writeln!(file, "    }}")?;
        }
    }

    // End outermost mod
    writeln!(file, "}}")
}
