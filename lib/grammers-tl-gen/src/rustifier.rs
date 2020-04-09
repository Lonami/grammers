// Copyright 2020 - developers of the `grammers` project.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// https://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or https://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

//! Several functions to "rustify" names.
//!
//! Each parsed type can have a corresponding "rusty" name, and
//! the method for it can be found in the corresponding submodule:
//!
//! * `type_name` for use after a type definition (`type FooBar`, `enum FooBar`).
//! * `qual_name` for the qualified type name (`crate::foo::BarBaz`).
//! * `variant_name` for use inside `enum` variants (`Foo`).
//! * `attr_name` for use as an attribute name (`foo_bar: ()`).

use grammers_tl_parser::tl::{Definition, Parameter, ParameterType, Type};

/// Get the rusty type name for a certain definition, excluding namespace.
fn rusty_type_name(name: &str) -> String {
    enum Casing {
        Upper,
        Lower,
        Preserve,
    }

    let name = if let Some(pos) = name.rfind('.') {
        &name[pos + 1..]
    } else {
        name
    };

    let mut result = String::with_capacity(name.len());

    name.chars().fold(Casing::Upper, |casing, c| {
        if c == '_' {
            return Casing::Upper;
        }

        match casing {
            Casing::Upper => {
                result.push(c.to_ascii_uppercase());
                Casing::Lower
            }
            Casing::Lower => {
                result.push(c.to_ascii_lowercase());
                if c.is_ascii_uppercase() {
                    Casing::Lower
                } else {
                    Casing::Preserve
                }
            }
            Casing::Preserve => {
                result.push(c);
                if c.is_ascii_uppercase() {
                    Casing::Lower
                } else {
                    Casing::Preserve
                }
            }
        }
    });

    result
}

pub mod definitions {
    use super::*;

    pub fn type_name(def: &Definition) -> String {
        rusty_type_name(&def.name)
    }

    pub fn qual_name(def: &Definition) -> String {
        let mut result = String::new();
        result.push_str("crate::types::");
        def.namespace.iter().for_each(|ns| {
            result.push_str(ns);
            result.push_str("::");
        });
        result.push_str(&rusty_type_name(&def.name));
        result
    }

    pub fn variant_name(def: &Definition) -> String {
        let name = rusty_type_name(&def.name);
        let ty_name = rusty_type_name(&def.ty.name);

        let variant = if name.starts_with(&ty_name) {
            &name[ty_name.len()..]
        } else {
            &name
        };

        match variant {
            "" => {
                // Use the name from the last uppercase letter
                &name[name
                    .as_bytes()
                    .into_iter()
                    .rposition(|c| c.is_ascii_uppercase())
                    .unwrap_or(0)..]
            }
            "Self" => {
                // Use the name from the second-to-last uppercase letter
                &name[name
                    .as_bytes()
                    .into_iter()
                    .take(name.len() - variant.len())
                    .rposition(|c| c.is_ascii_uppercase())
                    .unwrap_or(0)..]
            }
            _ => variant,
        }
        .to_string()
    }
}

pub mod parameters {
    use super::*;

    pub fn type_name(param: &Parameter) -> String {
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
                result.push_str(&types::type_name(ty));
                if flag.is_some() {
                    result.push('>');
                }
                result
            }
        }
    }

    pub fn qual_name(param: &Parameter) -> String {
        match &param.ty {
            ParameterType::Flags => "u32".into(),
            ParameterType::Normal { ty, flag } if flag.is_some() && ty.name == "true" => {
                "bool".into()
            }
            ParameterType::Normal { ty, .. } => {
                let mut result = String::new();
                if ty.generic_ref {
                    result.push_str("Vec::<u8>")
                } else {
                    result.push_str(&types::qual_name(&ty));
                    if let Some(arg) = &ty.generic_arg {
                        result.push_str("::<");
                        result.push_str(&types::qual_name(arg));
                        result.push('>');
                    }
                }
                result
            }
        }
    }

    pub fn attr_name(param: &Parameter) -> String {
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
}

pub mod types {
    use super::*;

    pub fn type_name(ty: &Type) -> String {
        rusty_type_name(&ty.name)
    }

    /// Sanitizes a name to be legal.
    pub fn push_sanitized_name(result: &mut String, ty: &Type) {
        let base = match ty.name.as_ref() {
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
            result.push_str(&qual_name(ty));
        } else {
            result.push_str(base);
        }
    }

    /// Sanitizes a path to be legal.
    pub fn push_sanitized_path(result: &mut String, ty: &Type) {
        // All sanitized names are valid paths except for a few base cases.
        let base = match ty.name.as_ref() {
            "bytes" => "Vec::<u8>",
            "int128" => "<[u8; 16]>",
            "int256" => "<[u8; 32]>",
            _ => "",
        };

        if base.is_empty() {
            push_sanitized_name(result, ty);
        } else {
            result.push_str(base);
        }
    }

    /// Get the rusty type name for a certain type.
    pub fn rusty_type(ty: &Type) -> String {
        let mut result = String::new();
        if ty.generic_ref {
            result.push_str("crate::Blob")
        } else {
            push_sanitized_name(&mut result, ty);
            if let Some(arg) = &ty.generic_arg {
                result.push('<');
                push_sanitized_name(&mut result, arg);
                result.push('>');
            }
        }
        result
    }

    pub fn qual_name(ty: &Type) -> String {
        let mut result = String::new();
        if ty.bare {
            result.push_str("crate::types::");
        } else {
            result.push_str("crate::enums::");
        }
        ty.namespace.iter().for_each(|ns| {
            result.push_str(ns);
            result.push_str("::");
        });
        result.push_str(&rusty_type_name(&ty.name));
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use grammers_tl_parser::tl::Category;

    #[test]
    fn check_rusty_type_name() {
        assert_eq!(rusty_type_name("ns.some_OK_name"), "SomeOkName");
    }

    #[test]
    fn check_def_variant_name() {
        let def = "new_session_created = NewSession".parse().unwrap();
        let name = definitions::variant_name(&def);
        assert_eq!(name, "Created");
    }

    #[test]
    fn check_def_empty_variant_name() {
        let def = "true = True".parse().unwrap();
        let name = definitions::variant_name(&def);
        assert_eq!(name, "True");
    }

    #[test]
    fn check_def_self_variant_name() {
        let def = "inputPeerSelf = InputPeer".parse().unwrap();
        let name = definitions::variant_name(&def);
        assert_eq!(name, "PeerSelf");
    }

    // TODO test EVERY function that tl-types relies on to compile
}
