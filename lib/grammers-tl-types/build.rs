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
        .filter(|d| !d.ty.generic_ref)
        .for_each(|d| {
            let (ns, name) = if let Some(pos) = d.ty.name.find('.') {
                (&d.ty.name[..pos], &d.ty.name[pos + 1..])
            } else {
                ("", &d.ty.name[..])
            };

            result.entry(ns.into()).or_insert_with(Vec::new).push(name);
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
        if let Some(pos) = name.find('.') {
            let (ns, n) = (&name[..pos], &name[pos + 1..]);
            result.push_str(ns);
            result.push_str("::");
            result.push_str(&rusty_class_name(n));
        } else {
            result.push_str(&rusty_class_name(name));
        }
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
            push_sanitized_name(&mut result, &ty.name);
            if let Some(arg) = &ty.generic_arg {
                result.push('<');
                push_sanitized_name(&mut result, arg);
                result.push('>');
            }
            if flag.is_some() {
                result.push('>');
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

    writeln!(
        file,
        "{}impl crate::Identifiable for {} {{",
        indent,
        rusty_class_name(&def.name)
    )?;
    writeln!(
        file,
        "{}    fn constructor_id() -> u32 {{ {} }}",
        indent,
        def.id.unwrap()
    )?;
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
    writeln!(file, "{}pub enum {} {{", indent, rusty_class_name(name))?;
    for d in definitions
        .into_iter()
        .filter(|d| d.category == Category::Types && d.ty.name == name)
    {
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
        write!(file, "crate::types::{}", rusty_class_name(&d.name))?;
        if recurses {
            write!(file, ">")?;
        }

        writeln!(file, "),")?;
    }
    writeln!(file, "{}}}", indent)
}

fn main() -> std::io::Result<()> {
    let api = load_tl("tl/api.tl")?;
    let mtproto = load_tl("tl/mtproto.tl")?;

    let mut file = BufWriter::new(File::create("src/generated.rs")?);

    // Begin outermost mod
    writeln!(file, "pub mod types {{")?;

    let api_types = group_by_ns(&api, Category::Types);
    let mut sorted_keys: Vec<&String> = api_types.keys().collect();
    sorted_keys.sort();
    for key in sorted_keys.into_iter() {
        // Begin possibly inner mod
        let indent = if key.is_empty() {
            "    "
        } else {
            writeln!(file, "    pub mod {} {{", key)?;
            "        "
        };

        for definition in api_types[key].iter() {
            write_definition(&mut file, indent, definition)?;
        }

        // End possibly inner mod
        if !key.is_empty() {
            writeln!(file, "    }}")?;
        }
    }

    // End outermost mod
    writeln!(file, "}}")?;

    writeln!(file, "pub mod functions {{")?;
    writeln!(file, "}}")?;

    // Begin outermost mod
    writeln!(file, "pub mod enums {{")?;

    let api_types = group_types_by_ns(&api);
    let mut sorted_keys: Vec<&String> = api_types.keys().collect();
    sorted_keys.sort();
    for key in sorted_keys.into_iter() {
        // Begin possibly inner mod
        let indent = if key.is_empty() {
            "    "
        } else {
            writeln!(file, "    pub mod {} {{", key)?;
            "        "
        };

        for name in api_types[key].iter() {
            write_enum(&mut file, indent, name, &api)?;
        }

        // End possibly inner mod
        if !key.is_empty() {
            writeln!(file, "    }}")?;
        }
    }

    // End outermost mod
    writeln!(file, "}}")?;

    file.flush()?;

    Ok(())
}
