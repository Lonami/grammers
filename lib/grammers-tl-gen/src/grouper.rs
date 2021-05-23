// Copyright 2020 - developers of the `grammers` project.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// https://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or https://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

//! Several functions to group definitions by a certain criteria.

use grammers_tl_parser::tl::{Category, Definition, Type};
use std::collections::HashMap;

/// Group the input vector by namespace, filtering by a certain category.
pub(crate) fn group_by_ns(
    definitions: &[Definition],
    category: Category,
) -> HashMap<String, Vec<&Definition>> {
    let mut result = HashMap::new();
    definitions
        .iter()
        .filter(|d| d.category == category)
        .for_each(|d| {
            // We currently only handle zero or one namespace.
            assert!(d.namespace.len() <= 1);
            let ns = d.namespace.get(0).map(|x| &x[..]).unwrap_or("");
            result.entry(ns.into()).or_insert_with(Vec::new).push(d);
        });

    for (_, vec) in result.iter_mut() {
        vec.sort_by_key(|d| &d.name);
    }
    result
}

/// Similar to `group_by_ns`, but for the definition types.
pub(crate) fn group_types_by_ns(definitions: &[Definition]) -> HashMap<Option<String>, Vec<&Type>> {
    let mut result = HashMap::new();
    definitions
        .iter()
        .filter(|d| d.category == Category::Types && !d.ty.generic_ref)
        .for_each(|d| {
            // We currently only handle zero or one namespace.
            assert!(d.namespace.len() <= 1);
            result
                .entry(d.namespace.get(0).map(Clone::clone))
                .or_insert_with(Vec::new)
                .push(&d.ty);
        });

    for (_, vec) in result.iter_mut() {
        vec.sort_by_key(|t| &t.name);
        vec.dedup();
    }
    result
}
