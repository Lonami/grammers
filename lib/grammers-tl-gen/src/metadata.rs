// Copyright 2020 - developers of the `grammers` project.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// https://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or https://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.
use std::collections::{HashMap, HashSet};

use grammers_tl_parser::tl::{Category, ParameterType, Type};

use crate::GeneratableDefinition;

/// Additional metadata required by several parts of the generation.
pub(crate) struct Metadata<'a> {
    recursing_defs: HashSet<u32>,
    defs_with_type: HashMap<(&'a Vec<String>, &'a String), Vec<&'a GeneratableDefinition>>,
}

impl<'a> Metadata<'a> {
    pub fn new(definitions: &'a [GeneratableDefinition]) -> Self {
        let mut metadata = Self {
            recursing_defs: HashSet::new(),
            defs_with_type: HashMap::new(),
        };

        definitions
            .iter()
            .filter(|d| d.parsed.category == Category::Types)
            .for_each(|d| {
                if d.parsed.params.iter().any(|p| match &p.ty {
                    ParameterType::Flags => false,
                    ParameterType::Normal { ty, .. } => {
                        ty.namespace == d.parsed.ty.namespace && ty.name == d.parsed.ty.name
                    }
                }) {
                    metadata.recursing_defs.insert(d.parsed.id);
                }

                metadata
                    .defs_with_type
                    .entry((&d.parsed.ty.namespace, &d.parsed.ty.name))
                    .or_insert_with(Vec::new)
                    .push(d);
            });

        metadata
    }

    /// Returns `true` if any of the parameters of `Definition` are of the
    /// same type as the `Definition` itself (meaning it recurses).
    pub fn is_recursive_def(&self, def: &GeneratableDefinition) -> bool {
        self.recursing_defs.contains(&def.parsed.id)
    }

    pub fn defs_with_type(&self, ty: &'a Type) -> &Vec<&GeneratableDefinition> {
        &self.defs_with_type[&(&ty.namespace, &ty.name)]
    }
}
