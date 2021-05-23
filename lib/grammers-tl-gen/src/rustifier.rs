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
//! * `item_path` for use as a qualified item path (`Vec::<u8>`).
//! * `attr_name` for use as an attribute name (`foo_bar: ()`).

use grammers_tl_parser::tl::{Definition, Parameter, ParameterType, Type};

/// Get the rusty type name for a certain definition, excluding namespace.
///
/// For example, transforms `"ns.some_OK_name"` into `"SomeOkName"`.
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
        result.push_str(&type_name(def));
        result
    }

    pub fn variant_name(def: &Definition) -> String {
        let name = type_name(def);
        let ty_name = types::type_name(&def.ty);

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
                    .iter()
                    .rposition(|c| c.is_ascii_uppercase())
                    .unwrap_or(0)..]
            }
            "Self" => {
                // Use the name from the second-to-last uppercase letter
                &name[name
                    .as_bytes()
                    .iter()
                    .take(name.len() - variant.len())
                    .rposition(|c| c.is_ascii_uppercase())
                    .unwrap_or(0)..]
            }
            _ => variant,
        }
        .to_string()
    }
}

pub mod types {
    use super::*;

    fn builtin_type(ty: &Type, path: bool) -> Option<&'static str> {
        Some(match ty.name.as_ref() {
            "Bool" => "bool",
            "bytes" => {
                if path {
                    "Vec::<u8>"
                } else {
                    "Vec<u8>"
                }
            }
            "double" => "f64",
            "int" => "i32",
            "int128" => {
                if path {
                    "<[u8; 16]>"
                } else {
                    "[u8; 16]"
                }
            }
            "int256" => {
                if path {
                    "<[u8; 32]>"
                } else {
                    "[u8; 32]"
                }
            }
            "long" => "i64",
            "string" => "String",
            "true" => "bool",
            "vector" => "crate::RawVec",
            "Vector" => "Vec",
            _ => return None,
        })
    }

    // There are only minor differences between qualified
    // name and item paths so this method is used for both:
    // 1. use `::<...>` instead of `<...>` to specify type arguments
    // 2. missing angle brackets in associated item path
    fn get_path(ty: &Type, path: bool) -> String {
        if ty.generic_ref {
            return ty.name.clone();
        }

        let mut result = if let Some(name) = builtin_type(ty, path) {
            name.to_string()
        } else {
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
            result.push_str(&type_name(ty));
            result
        };

        if let Some(generic_ty) = &ty.generic_arg {
            if path {
                result.push_str("::");
            }
            result.push('<');
            result.push_str(&qual_name(generic_ty));
            result.push('>');
        }

        result
    }

    pub fn type_name(ty: &Type) -> String {
        rusty_type_name(&ty.name)
    }

    pub fn qual_name(ty: &Type) -> String {
        get_path(ty, false)
    }

    pub fn item_path(ty: &Type) -> String {
        get_path(ty, true)
    }
}

pub mod parameters {
    use super::*;

    pub fn qual_name(param: &Parameter) -> String {
        match &param.ty {
            ParameterType::Flags => "u32".into(),
            ParameterType::Normal { ty, flag } if flag.is_some() && ty.name == "true" => {
                "bool".into()
            }
            ParameterType::Normal { ty, flag } => {
                let mut result = String::new();
                if flag.is_some() {
                    result.push_str("Option<");
                }
                result.push_str(&types::qual_name(ty));
                if flag.is_some() {
                    result.push('>');
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

#[cfg(test)]
mod tests {
    use super::*;

    // Core methods

    #[test]
    fn check_rusty_type_name() {
        assert_eq!(rusty_type_name("ns.some_OK_name"), "SomeOkName");
    }

    // Definition methods

    #[test]
    fn check_def_type_name() {
        let def = "userEmpty = User".parse().unwrap();
        let name = definitions::type_name(&def);
        assert_eq!(name, "UserEmpty");
    }

    #[test]
    fn check_def_qual_name() {
        let def = "userEmpty = User".parse().unwrap();
        let name = definitions::qual_name(&def);
        assert_eq!(name, "crate::types::UserEmpty");
    }

    #[test]
    fn check_def_namespaced_qual_name() {
        let def = "upload.fileCdnRedirect = upload.File".parse().unwrap();
        let name = definitions::qual_name(&def);
        assert_eq!(name, "crate::types::upload::FileCdnRedirect");
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

    // Type methods

    #[test]
    fn check_type_type_name() {
        let ty = "storage.FileType".parse().unwrap();
        let name = types::type_name(&ty);
        assert_eq!(name, "FileType");
    }

    #[test]
    fn check_type_qual_name() {
        let ty = "InputPeer".parse().unwrap();
        let name = types::qual_name(&ty);
        assert_eq!(name, "crate::enums::InputPeer");
    }

    #[test]
    fn check_type_qual_namespaced_name() {
        let ty = "storage.FileType".parse().unwrap();
        let name = types::qual_name(&ty);
        assert_eq!(name, "crate::enums::storage::FileType");
    }

    #[test]
    fn check_type_qual_bare_name() {
        let ty = "ipPort".parse().unwrap();
        let name = types::qual_name(&ty);
        assert_eq!(name, "crate::types::IpPort");
    }

    #[test]
    fn check_type_qual_namespaced_bare_name() {
        let ty = "storage.fileUnknown".parse().unwrap();
        let name = types::qual_name(&ty);
        assert_eq!(name, "crate::types::storage::FileUnknown");
    }

    #[test]
    fn check_type_bytes_qual_name() {
        let ty = "bytes".parse().unwrap();
        let name = types::qual_name(&ty);
        assert_eq!(name, "Vec<u8>");
    }

    #[test]
    fn check_type_large_int_qual_name() {
        let ty = "int256".parse().unwrap();
        let name = types::qual_name(&ty);
        assert_eq!(name, "[u8; 32]");
    }

    #[test]
    fn check_type_raw_vec_qual_name() {
        let ty = "vector<long>".parse().unwrap();
        let name = types::qual_name(&ty);
        assert_eq!(name, "crate::RawVec<i64>");
    }

    #[test]
    fn check_type_vec_qual_name() {
        let ty = "Vector<Bool>".parse().unwrap();
        let name = types::qual_name(&ty);
        assert_eq!(name, "Vec<bool>");
    }

    #[test]
    fn check_type_generic_ref_qual_name() {
        let mut ty: Type = "X".parse().unwrap();
        ty.generic_ref = true;
        let name = types::qual_name(&ty);
        assert_eq!(name, "X");
    }

    #[test]
    fn check_type_item_path() {
        let ty = "Vector<FileHash>".parse().unwrap();
        let name = types::item_path(&ty);
        assert_eq!(name, "Vec::<crate::enums::FileHash>");
    }

    #[test]
    fn check_type_bytes_item_path() {
        let ty = "bytes".parse().unwrap();
        let name = types::item_path(&ty);
        assert_eq!(name, "Vec::<u8>");
    }

    #[test]
    fn check_type_large_int_item_path() {
        let ty = "int256".parse().unwrap();
        let name = types::item_path(&ty);
        assert_eq!(name, "<[u8; 32]>");
    }

    // Parameter methods

    #[test]
    fn check_param_qual_name() {
        let param = "pts:int".parse().unwrap();
        let name = parameters::qual_name(&param);
        assert_eq!(name, "i32");
    }

    #[test]
    fn check_param_flag_def_qual_name() {
        let param = "flags:#".parse().unwrap();
        let name = parameters::qual_name(&param);
        assert_eq!(name, "u32");
    }

    #[test]
    fn check_param_flags_qual_name() {
        let param = "timeout:flags.1?int".parse().unwrap();
        let name = parameters::qual_name(&param);
        assert_eq!(name, "Option<i32>");
    }

    #[test]
    fn check_param_true_flags_qual_name() {
        let param = "big:flags.0?true".parse().unwrap();
        let name = parameters::qual_name(&param);
        assert_eq!(name, "bool");
    }

    #[test]
    fn check_param_attr_name() {
        let param = "access_hash:long".parse().unwrap();
        let name = parameters::attr_name(&param);
        assert_eq!(name, "access_hash");
    }

    #[test]
    fn check_param_reserved_attr_name() {
        let param = "final:flags.0?true".parse().unwrap();
        let name = parameters::attr_name(&param);
        assert_eq!(name, "r#final");
    }

    #[test]
    fn check_param_self_attr_name() {
        let param = "self:flags.0?true".parse().unwrap();
        let name = parameters::attr_name(&param);
        assert_eq!(name, "is_self");
    }
}
