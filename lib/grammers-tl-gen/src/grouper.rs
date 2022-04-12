// Copyright 2020 - developers of the `grammers` project.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// https://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or https://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

//! Several functions to group definitions by a certain criteria.

use grammers_tl_parser::tl::{Category, Type};
use std::collections::HashMap;

use crate::GeneratableDefinition;

/// Group the input vector by namespace, filtering by a certain category.
pub(crate) fn group_by_ns(
    definitions: &[GeneratableDefinition],
    category: Category,
) -> HashMap<String, Vec<&GeneratableDefinition>> {
    let mut result = HashMap::new();
    definitions
        .iter()
        .filter(|d| d.parsed.category == category)
        .for_each(|d| {
            // We currently only handle zero or one namespace.
            assert!(d.parsed.namespace.len() <= 1);
            let ns = d.parsed.namespace.get(0).map(|x| &x[..]).unwrap_or("");
            result.entry(ns.into()).or_insert_with(Vec::new).push(d);
        });

    for (_, vec) in result.iter_mut() {
        vec.sort_by_key(|d| &d.parsed.name);
    }
    result
}

/// Similar to `group_by_ns`, but for the definition types.
pub(crate) fn group_types_by_ns(definitions: &[GeneratableDefinition]) -> HashMap<Option<String>, Vec<&Type>> {
    let mut result = HashMap::new();
    definitions
        .iter()
        .filter(|d| d.parsed.category == Category::Types && !d.parsed.ty.generic_ref)
        .for_each(|d| {
            // We currently only handle zero or one namespace.
            assert!(d.parsed.namespace.len() <= 1);
            result
                .entry(d.parsed.namespace.get(0).map(Clone::clone))
                .or_insert_with(Vec::new)
                .push(&d.parsed.ty);
        });

    for (_, vec) in result.iter_mut() {
        vec.sort_by_key(|t| &t.name);
        vec.dedup();
    }
    result
}
