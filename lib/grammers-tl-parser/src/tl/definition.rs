// Copyright 2020 - developers of the `grammers` project.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// https://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or https://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.
use std::fmt;
use std::str::FromStr;

use crate::errors::{ParamParseError, ParseError};
use crate::tl::{Category, Flag, Parameter, ParameterType, Type};
use crate::utils::infer_id;

/// A [Type Language] definition.
///
/// [Type Language]: https://core.telegram.org/mtproto/TL
#[derive(Debug, PartialEq)]
pub struct Definition {
    /// The namespace components of the definition. This list will be empty
    /// if the name of the definition belongs to the global namespace.
    pub namespace: Vec<String>,

    /// The name of this definition. Also known as "predicate" or "method".
    pub name: String,

    /// The numeric identifier of this definition.
    ///
    /// If a definition has an identifier, it overrides this value.
    /// Otherwise, the identifier is inferred from the definition.
    pub id: u32,

    /// A possibly-empty list of parameters this definition has.
    pub params: Vec<Parameter>,

    /// The type to which this definition belongs to.
    pub ty: Type,

    /// The category to which this definition belongs to.
    pub category: Category,
}

impl fmt::Display for Definition {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for ns in self.namespace.iter() {
            write!(f, "{}.", ns)?;
        }
        write!(f, "{}#{:x}", self.name, self.id)?;

        // If any parameter references a generic, make sure to define it early
        let mut type_defs = vec![];
        for param in self.params.iter() {
            if let ParameterType::Normal { ty, .. } = &param.ty {
                ty.find_generic_refs(&mut type_defs);
            }
        }
        type_defs.sort_unstable();
        type_defs.dedup();
        for type_def in type_defs {
            write!(f, " {{{}:Type}}", type_def)?;
        }

        for param in self.params.iter() {
            write!(f, " {}", param)?;
        }
        write!(f, " = {}", self.ty)?;
        Ok(())
    }
}

impl FromStr for Definition {
    type Err = ParseError;

    /// Parses a [Type Language] definition.
    ///
    /// # Examples
    ///
    /// ```
    /// use grammers_tl_parser::tl::Definition;
    ///
    /// assert!("sendMessage chat_id:int message:string = Message".parse::<Definition>().is_ok());
    /// ```
    ///
    /// [Type Language]: https://core.telegram.org/mtproto/TL
    fn from_str(definition: &str) -> Result<Self, Self::Err> {
        if definition.trim().is_empty() {
            return Err(ParseError::Empty);
        }

        // Parse `(left = ty)`
        let (left, ty) = {
            let mut it = definition.split('=');
            let ls = it.next().unwrap(); // split() always return at least one
            if let Some(t) = it.next() {
                (ls.trim(), t.trim())
            } else {
                return Err(ParseError::MissingType);
            }
        };

        let mut ty = Type::from_str(ty).map_err(|_| ParseError::MissingType)?;

        // Parse `name middle`
        let (name, middle) = {
            if let Some(pos) = left.find(' ') {
                (&left[..pos], left[pos..].trim())
            } else {
                (left.trim(), "")
            }
        };

        // Parse `name#id`
        let (name, id) = {
            let mut it = name.split('#');
            let n = it.next().unwrap(); // split() always return at least one
            (n, it.next())
        };

        // Parse `ns1.ns2.name`
        let mut namespace: Vec<String> = name.split('.').map(|part| part.to_string()).collect();
        if namespace.iter().any(|part| part.is_empty()) {
            return Err(ParseError::MissingName);
        }

        // Safe to unwrap because split() will always yield at least one.
        let name = namespace.pop().unwrap();

        // Parse `id`
        let id = match id {
            Some(v) => u32::from_str_radix(v.trim(), 16).map_err(ParseError::InvalidId)?,
            None => infer_id(definition),
        };

        // Parse `middle`
        let mut type_defs = vec![];
        let mut flag_defs = vec![];

        let params = middle
            .split_whitespace()
            .map(Parameter::from_str)
            .filter_map(|p| match p {
                // If the parameter is a type definition save it
                // and ignore this parameter.
                Err(ParamParseError::TypeDef { name }) => {
                    type_defs.push(name);
                    None
                }

                // If the parameter is a flag definition save both
                // the definition and the parameter.
                Ok(Parameter {
                    ref name,
                    ty: ParameterType::Flags,
                }) => {
                    flag_defs.push(name.clone());
                    Some(Ok(p.unwrap()))
                }

                // If the parameter type is a generic ref ensure it's valid.
                Ok(Parameter {
                    ty:
                        ParameterType::Normal {
                            ty:
                                Type {
                                    ref name,
                                    generic_ref,
                                    ..
                                },
                            ..
                        },
                    ..
                }) if generic_ref => {
                    if generic_ref && !type_defs.contains(&name) {
                        Some(Err(ParseError::InvalidParam(ParamParseError::MissingDef)))
                    } else {
                        Some(Ok(p.unwrap()))
                    }
                }

                // If the parameter type references a flag ensure it's valid
                Ok(Parameter {
                    ty:
                        ParameterType::Normal {
                            flag: Some(Flag { ref name, .. }),
                            ..
                        },
                    ..
                }) => {
                    if !flag_defs.contains(&&name) {
                        Some(Err(ParseError::InvalidParam(ParamParseError::MissingDef)))
                    } else {
                        Some(Ok(p.unwrap()))
                    }
                }

                // Any other parameter that's okay should just be passed as-is.
                Ok(p) => Some(Ok(p)),

                // Unimplenented parameters are unimplemented definitions.
                Err(ParamParseError::NotImplemented) => Some(Err(ParseError::NotImplemented)),

                // Any error should just become a `ParseError`
                Err(x) => Some(Err(ParseError::InvalidParam(x))),
            })
            .collect::<Result<_, ParseError>>()?;

        // The type lacks `!` so we determine if it's a generic one based
        // on whether its name is known in a previous parameter type def.
        if type_defs.contains(&ty.name) {
            ty.generic_ref = true;
        }

        Ok(Definition {
            namespace,
            name,
            id,
            params,
            ty,
            category: Category::Types,
        })
    }
}

impl Definition {
    /// Convenience function to format both the namespace and name back into a single string.
    pub fn full_name(&self) -> String {
        let mut result = String::with_capacity(
            self.namespace.iter().map(|ns| ns.len() + 1).sum::<usize>() + self.name.len(),
        );
        for ns in self.namespace.iter() {
            result.push_str(ns);
            result.push('.');
        }
        result.push_str(&self.name);
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tl::Flag;

    #[test]
    fn parse_empty_def() {
        assert_eq!(Definition::from_str(""), Err(ParseError::Empty));
    }

    #[test]
    fn parse_bad_id() {
        let bad = u32::from_str_radix("bar", 16).unwrap_err();
        let bad_q = u32::from_str_radix("?", 16).unwrap_err();
        let bad_empty = u32::from_str_radix("", 16).unwrap_err();
        assert_eq!(
            Definition::from_str("foo#bar = baz"),
            Err(ParseError::InvalidId(bad))
        );
        assert_eq!(
            Definition::from_str("foo#? = baz"),
            Err(ParseError::InvalidId(bad_q))
        );
        assert_eq!(
            Definition::from_str("foo# = baz"),
            Err(ParseError::InvalidId(bad_empty))
        );
    }

    #[test]
    fn parse_no_name() {
        assert_eq!(Definition::from_str(" = foo"), Err(ParseError::MissingName));
    }

    #[test]
    fn parse_no_type() {
        assert_eq!(Definition::from_str("foo"), Err(ParseError::MissingType));
        assert_eq!(Definition::from_str("foo = "), Err(ParseError::MissingType));
    }

    #[test]
    fn parse_unimplemented() {
        assert_eq!(
            Definition::from_str("int ? = Int"),
            Err(ParseError::NotImplemented)
        );
    }

    #[test]
    fn parse_override_id() {
        let def = "rpc_answer_dropped msg_id:long seq_no:int bytes:int = RpcDropAnswer";
        assert_eq!(Definition::from_str(def).unwrap().id, 0xa43ad8b7);

        let def = "rpc_answer_dropped#123456 msg_id:long seq_no:int bytes:int = RpcDropAnswer";
        assert_eq!(Definition::from_str(def).unwrap().id, 0x123456);
    }

    #[test]
    fn parse_valid_definition() {
        let def = Definition::from_str("a#1=d").unwrap();
        assert_eq!(def.name, "a");
        assert_eq!(def.id, 1);
        assert_eq!(def.params.len(), 0);
        assert_eq!(
            def.ty,
            Type {
                namespace: vec![],
                name: "d".into(),
                bare: true,
                generic_ref: false,
                generic_arg: None,
            }
        );

        let def = Definition::from_str("a=d<e>").unwrap();
        assert_eq!(def.name, "a");
        assert_ne!(def.id, 0);
        assert_eq!(def.params.len(), 0);
        assert_eq!(
            def.ty,
            Type {
                namespace: vec![],
                name: "d".into(),
                bare: true,
                generic_ref: false,
                generic_arg: Some(Box::new("e".parse().unwrap())),
            }
        );

        let def = Definition::from_str("a b:c = d").unwrap();
        assert_eq!(def.name, "a");
        assert_ne!(def.id, 0);
        assert_eq!(def.params.len(), 1);
        assert_eq!(
            def.ty,
            Type {
                namespace: vec![],
                name: "d".into(),
                bare: true,
                generic_ref: false,
                generic_arg: None,
            }
        );

        let def = Definition::from_str("a#1 {b:Type} c:!b = d").unwrap();
        assert_eq!(def.name, "a");
        assert_eq!(def.id, 1);
        assert_eq!(def.params.len(), 1);
        assert!(match def.params[0].ty {
            ParameterType::Normal {
                ty: Type { generic_ref, .. },
                ..
            } if generic_ref => true,
            _ => false,
        });
        assert_eq!(
            def.ty,
            Type {
                namespace: vec![],
                name: "d".into(),
                bare: true,
                generic_ref: false,
                generic_arg: None,
            }
        );
    }

    #[test]
    fn parse_multiline_definition() {
        let def = "
            first#1 lol:param
              = t;
            ";

        assert_eq!(Definition::from_str(def).unwrap().id, 1);

        let def = "
            second#2
              lol:String
            = t;
            ";

        assert_eq!(Definition::from_str(def).unwrap().id, 2);

        let def = "
            third#3

              lol:String

            =
                     t;
            ";

        assert_eq!(Definition::from_str(def).unwrap().id, 3);
    }

    #[test]
    fn parse_complete() {
        let def = "ns1.name#123 {X:Type} flags:# pname:flags.10?ns2.Vector<!X> = ns3.Type";
        assert_eq!(
            Definition::from_str(def),
            Ok(Definition {
                namespace: vec!["ns1".into()],
                name: "name".into(),
                id: 0x123,
                params: vec![
                    Parameter {
                        name: "flags".into(),
                        ty: ParameterType::Flags,
                    },
                    Parameter {
                        name: "pname".into(),
                        ty: ParameterType::Normal {
                            ty: Type {
                                namespace: vec!["ns2".into()],
                                name: "Vector".into(),
                                bare: false,
                                generic_ref: false,
                                generic_arg: Some(Box::new(Type {
                                    namespace: vec![],
                                    name: "X".into(),
                                    bare: false,
                                    generic_ref: true,
                                    generic_arg: None,
                                })),
                            },
                            flag: Some(Flag {
                                name: "flags".into(),
                                index: 10
                            })
                        },
                    },
                ],
                ty: Type {
                    namespace: vec!["ns3".into()],
                    name: "Type".into(),
                    bare: false,
                    generic_ref: false,
                    generic_arg: None,
                },
                category: Category::Types,
            })
        );
    }

    #[test]
    fn parse_missing_generic() {
        let def = "name param:!X = Type";
        assert_eq!(
            Definition::from_str(def),
            Err(ParseError::InvalidParam(ParamParseError::MissingDef))
        );

        let def = "name {X:Type} param:!Y = Type";
        assert_eq!(
            Definition::from_str(def),
            Err(ParseError::InvalidParam(ParamParseError::MissingDef))
        );
    }

    #[test]
    fn parse_unknown_flags() {
        let def = "name param:flags.0?true = Type";
        assert_eq!(
            Definition::from_str(def),
            Err(ParseError::InvalidParam(ParamParseError::MissingDef))
        );

        let def = "name foo:# param:flags.0?true = Type";
        assert_eq!(
            Definition::from_str(def),
            Err(ParseError::InvalidParam(ParamParseError::MissingDef))
        );
    }

    #[test]
    fn test_to_string() {
        let def = "ns1.name#123 {X:Type} flags:# pname:flags.10?ns2.Vector<!X> = ns3.Type";
        assert_eq!(Definition::from_str(def).unwrap().to_string(), def);
    }
}
