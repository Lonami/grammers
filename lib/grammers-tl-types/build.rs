use grammers_tl_parser::{parse_tl_file, Category, Definition, Parameter, ParameterType};
use std::collections::HashMap;
use std::fs::File;
use std::io::prelude::*;
use std::io::{self, BufWriter};

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

/// Group the input vector by namespace, filtering by a certain category.
fn group_by_ns(
    definitions: &Vec<Definition>,
    category: Category,
) -> HashMap<String, Vec<&Definition>> {
    let mut result = HashMap::new();
    definitions
        .into_iter()
        .filter(|d| d.category == category)
        .for_each(|d| {
            let ns = if let Some(pos) = d.name.find('.') {
                &d.name[..pos]
            } else {
                ""
            };

            result.entry(ns.into()).or_insert_with(Vec::new).push(d);
        });

    for (_, vec) in result.iter_mut() {
        vec.sort_by_key(|d| &d.name);
    }
    result
}

/// Similar to `group_by_ns`, but for the definition types.
fn group_types_by_ns(definitions: &Vec<Definition>) -> HashMap<String, Vec<&str>> {
    let mut result = HashMap::new();
    definitions
        .into_iter()
        .filter(|d| d.category == Category::Types && !d.ty.generic_ref)
        .for_each(|d| {
            let ns = if let Some(pos) = d.ty.name.find('.') {
                &d.ty.name[..pos]
            } else {
                ""
            };

            result
                .entry(ns.into())
                .or_insert_with(Vec::new)
                .push(&d.ty.name[..]);
        });

    for (_, vec) in result.iter_mut() {
        vec.sort();
        vec.dedup();
    }
    result
}

/// Get the rusty class name for a certain definition, excluding namespace.
fn rusty_class_name(name: &str) -> String {
    let mut name: String = if let Some(pos) = name.find('.') {
        &name[pos + 1..]
    } else {
        name
    }
    .into();

    name[..1].make_ascii_uppercase();
    name
}

/// Get a rusty class name, including namespaces.
fn rusty_namespaced_class_name(name: &str) -> String {
    let mut result = String::new();
    if let Some(pos) = name.find('.') {
        let (ns, n) = (&name[..pos], &name[pos + 1..]);
        result.push_str(ns);
        result.push_str("::");
        result.push_str(&rusty_class_name(n));
    } else {
        result.push_str(&rusty_class_name(name));
    }
    result
}

/// Get the rusty attribute name for a certain parameter.
fn rusty_attr_name(param: &Parameter) -> String {
    match &param.name[..] {
        "final" => "r#final".into(),
        "loop" => "r#loop".into(),
        "self" => "is_self".into(),
        "static" => "r#static".into(),
        "type" => "r#type".into(),
        _ => {
            let mut result = param.name.clone();
            result[..].make_ascii_lowercase();
            result
        }
    }
}

/// Sanitizes a name to be legal.
fn push_sanitized_name(result: &mut String, name: &str) {
    let base = match name {
        "Bool" => "bool",
        "bytes" => "Vec<u8>",
        "double" => "f64",
        "int" => "i32",
        "int128" => "[u8; 16]",
        "int256" => "[u8; 32]",
        "long" => "i64",
        "string" => "String",
        "true" => "bool",
        "Vector" => "Vec",
        _ => "",
    };
    if base.is_empty() {
        result.push_str("crate::enums::");
        result.push_str(&rusty_namespaced_class_name(name));
    } else {
        result.push_str(base);
    }
}

/// Get the rusty type name for a certain parameter.
fn rusty_type_name(param: &Parameter) -> String {
    match &param.ty {
        ParameterType::Flags => "u32".into(),
        ParameterType::Normal { ty, flag } if flag.is_some() && ty.name == "true" => {
            // Special-case: `flags.i?true` are just `bool`.
            "bool".into()
        }
        ParameterType::Normal { ty, flag } => {
            let mut result = String::new();
            if flag.is_some() {
                result.push_str("Option<");
            }

            // Special-case: generic references can represent any type.
            //
            // Using an array of bytes lets us store any data without
            // caring about the type (no generics is also more FFI-friendly).
            if ty.generic_ref {
                result.push_str("Vec<u8>")
            } else {
                push_sanitized_name(&mut result, &ty.name);
                if let Some(arg) = &ty.generic_arg {
                    result.push('<');
                    push_sanitized_name(&mut result, arg);
                    result.push('>');
                }
            }
            if flag.is_some() {
                result.push('>');
            }
            result
        }
    }
}

/// Similar to `rusty_type_name` but to access a path
/// (for instance `Vec::<u8>` and not `Vec<u8>`).
/// Note that optionals don't get special treatment.
fn rusty_type_path(param: &Parameter) -> String {
    match &param.ty {
        ParameterType::Flags => "u32".into(),
        ParameterType::Normal { ty, flag } if flag.is_some() && ty.name == "true" => "bool".into(),
        ParameterType::Normal { ty, .. } => {
            let mut result = String::new();
            if ty.generic_ref {
                result.push_str("Vec::<u8>")
            } else {
                push_sanitized_name(&mut result, &ty.name);
                if let Some(arg) = &ty.generic_arg {
                    result.push_str("::<");
                    push_sanitized_name(&mut result, arg);
                    result.push('>');
                }
            }
            result
        }
    }
}

/// Writes a definition such as the following rust code:
///
/// ```
/// pub struct Name {
///     pub field: Type,
/// }
/// impl crate::Identifiable for Name {
///     fn constructor_id() -> u32 { 123 }
/// }
/// ```
fn write_definition<W: Write>(file: &mut W, indent: &str, def: &Definition) -> io::Result<()> {
    let class_name = rusty_class_name(&def.name);

    // Define struct
    writeln!(file, "{}pub struct {} {{", indent, class_name)?;
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

    // impl Identifiable
    writeln!(
        file,
        "{}impl crate::Identifiable for {} {{",
        indent, class_name
    )?;
    writeln!(
        file,
        "{}    fn constructor_id() -> u32 {{ {} }}",
        indent,
        def.id.unwrap()
    )?;
    writeln!(file, "{}}}", indent)?;

    // impl Serializable
    writeln!(
        file,
        "{}impl crate::Serializable for {} {{",
        indent, class_name
    )?;
    writeln!(
        file,
        "{}    fn serialize<B: std::io::Write>(&self, {}buf: &mut B) -> std::io::Result<()> {{",
        indent,
        if def.params.is_empty() { "_" } else { "" }
    )?;

    for param in def.params.iter() {
        write!(file, "{}        ", indent)?;
        match &param.ty {
            ParameterType::Flags => {
                write!(file, "buf.write(&(0u32")?;

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

                writeln!(file, ").to_le_bytes())?;")?;
            }
            ParameterType::Normal { ty, flag } => {
                // The `true` type is not serialized
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

    // impl Deserializable
    writeln!(
        file,
        "{}impl crate::Deserializable for {} {{",
        indent, class_name
    )?;
    writeln!(
        file,
        "{}    fn deserialize<B: std::io::Read>({}buf: &mut B) -> std::io::Result<Self> {{",
        indent,
        if def.params.is_empty() { "_" } else { "" }
    )?;

    for param in def.params.iter() {
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
                    if ty.name == "bytes" {
                        write!(file, "crate::Bytes::deserialize(buf)?.0")?;
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

    writeln!(file, "{}        Ok({} {{", indent, class_name)?;

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
    writeln!(file, "{}}}", indent)
}

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
    definitions: &Vec<Definition>,
) -> io::Result<()> {
    let class_name = rusty_class_name(name);

    let type_defs: Vec<&Definition> = definitions
        .into_iter()
        .filter(|d| d.category == Category::Types && d.ty.name == name)
        .collect();

    // Define enum
    writeln!(file, "{}pub enum {} {{", indent, class_name)?;
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

    // impl Serializable
    writeln!(
        file,
        "{}impl crate::Serializable for {} {{",
        indent, class_name
    )?;
    writeln!(
        file,
        "{}    fn serialize<B: std::io::Write>(&self, {}buf: &mut B) -> std::io::Result<()> {{",
        indent,
        if type_defs.is_empty() { "_" } else { "" }
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
                "{}                crate::types::{}::constructor_id().serialize(buf)?;",
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

    // impl Deserializable
    writeln!(
        file,
        "{}impl crate::Deserializable for {} {{",
        indent, class_name
    )?;
    writeln!(
        file,
        "{}    fn deserialize<B: std::io::Read>(_buf: &mut B) -> std::io::Result<Self> {{",
        indent,
    )?;
    writeln!(file, "{}        unimplemented!();", indent)?;
    writeln!(file, "{}    }}", indent)?;
    writeln!(file, "{}}}", indent)
}

/// Write an entire module for the desired category.
fn write_category_mod<W: Write>(
    mut file: &mut W,
    category: Category,
    definitions: &Vec<Definition>,
) -> io::Result<()> {
    // Begin outermost mod
    writeln!(
        file,
        "pub mod {} {{",
        match category {
            Category::Types => "types",
            Category::Functions => "functions",
        }
    )?;

    let grouped = group_by_ns(definitions, category);
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

        for definition in grouped[key].iter() {
            write_definition(&mut file, indent, definition)?;
        }

        // End possibly inner mod
        if !key.is_empty() {
            writeln!(file, "    }}")?;
        }
    }

    // End outermost mod
    writeln!(file, "}}")
}

/// Write the entire module dedicated to enums.
fn write_enums_mod<W: Write>(mut file: &mut W, definitions: &Vec<Definition>) -> io::Result<()> {
    // Begin outermost mod
    writeln!(file, "pub mod enums {{")?;

    let grouped = group_types_by_ns(definitions);
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
            write_enum(&mut file, indent, name, definitions)?;
        }

        // End possibly inner mod
        if !key.is_empty() {
            writeln!(file, "    }}")?;
        }
    }

    // End outermost mod
    writeln!(file, "}}")
}

fn main() -> std::io::Result<()> {
    let api = load_tl("tl/api.tl")?;
    let _mtproto = load_tl("tl/mtproto.tl")?; // TODO use

    let mut file = BufWriter::new(File::create("src/generated.rs")?);

    write_category_mod(&mut file, Category::Types, &api)?;
    write_category_mod(&mut file, Category::Functions, &api)?;
    write_enums_mod(&mut file, &api)?;

    file.flush()?;

    Ok(())
}
