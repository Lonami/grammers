//! Several functions to "rustify" names.

use grammers_tl_parser::{Parameter, ParameterType, Type};

/// Get the rusty class name for a certain definition, excluding namespace.
///
/// # Examples
///
/// ```
/// assert_eq!(rusty_class_name("ns.some_OK_name"), "SomeOKName");
/// ```
pub(crate) fn rusty_class_name(name: &str) -> String {
    let start = name.find('.').map(|p| p + 1).unwrap_or(0);
    let mut result = String::with_capacity(name.len() - start);

    name.chars().skip(start).fold(true, |upper, c| {
        if c == '_' {
            true
        } else if upper {
            result.push(c.to_ascii_uppercase());
            false
        } else {
            result.push(c);
            false
        }
    });

    result
}

/// Get a rusty class name, including namespaces.
pub(crate) fn rusty_namespaced_class_name(name: &str) -> String {
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
pub(crate) fn rusty_attr_name(param: &Parameter) -> String {
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
pub(crate) fn push_sanitized_name(result: &mut String, name: &str) {
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
        "vector" => "crate::RawVec",
        "Vector" => "Vec",
        _ => "",
    };
    if base.is_empty() {
        let first_letter = name.find('.').map(|p| p + 1).unwrap_or(0);
        if name.as_bytes()[first_letter].is_ascii_lowercase() {
            // Bare type
            result.push_str("crate::types::")
        } else {
            // Boxed type
            result.push_str("crate::enums::");
        }
        result.push_str(&rusty_namespaced_class_name(name));
    } else {
        result.push_str(base);
    }
}

/// Sanitizes a path to be legal.
pub(crate) fn push_sanitized_path(result: &mut String, name: &str) {
    // All sanitized names are valid paths except for a few base cases.
    let base = match name {
        "bytes" => "Vec::<u8>",
        "int128" => "<[u8; 16]>",
        "int256" => "<[u8; 32]>",
        _ => "",
    };

    if base.is_empty() {
        push_sanitized_name(result, name);
    } else {
        result.push_str(base);
    }
}

/// Get the rusty type name for a certain type.
pub(crate) fn rusty_type(ty: &Type) -> String {
    let mut result = String::new();
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
    result
}

/// Get the rusty type name for a certain parameter.
pub(crate) fn rusty_type_name(param: &Parameter) -> String {
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
            result.push_str(&rusty_type(ty));
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
pub(crate) fn rusty_type_path(param: &Parameter) -> String {
    match &param.ty {
        ParameterType::Flags => "u32".into(),
        ParameterType::Normal { ty, flag } if flag.is_some() && ty.name == "true" => "bool".into(),
        ParameterType::Normal { ty, .. } => {
            let mut result = String::new();
            if ty.generic_ref {
                result.push_str("Vec::<u8>")
            } else {
                push_sanitized_path(&mut result, &ty.name);
                if let Some(arg) = &ty.generic_arg {
                    result.push_str("::<");
                    push_sanitized_path(&mut result, arg);
                    result.push('>');
                }
            }
            result
        }
    }
}
