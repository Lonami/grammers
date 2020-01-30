use std::collections::HashSet;

use grammers_tl_parser::tl::{Definition, ParameterType};

/// Additional metadata required by several parts of the generation.
pub(crate) struct Metadata<'a> {
    recursing_defs: HashSet<u32>,
}

impl Metadata {
    pub fn new(definitions: &[Definition]) -> Self {
        let mut metadata = Self {
            recursing_defs: HashSet::new(),
        };

        definitions.into_iter().for_each(|d| {
            if d.params.iter().any(|p| match &p.ty {
                ParameterType::Flags => false,
                ParameterType::Normal { ty, .. } => {
                    ty.namespace == d.ty.namespace && ty.name == d.ty.name
                }
            }) {
                metadata.recursing_defs.insert(d.id);
            }
        });

        metadata
    }

    /// Returns `true` if any of the parameters of `Definition` are of the
    /// same type as the `Definition` itself (meaning it recurses).
    pub fn is_recursive_def(&self, def: &Definition) -> bool {
        self.recursing_defs.contains(&def.id)
    }
}
