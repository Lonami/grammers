//! Several functions to group definitions by a certain criteria.

use grammers_tl_parser::{Category, Definition};
use std::collections::HashMap;

/// Group the input vector by namespace, filtering by a certain category.
pub(crate) fn group_by_ns(
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
pub(crate) fn group_types_by_ns(definitions: &Vec<Definition>) -> HashMap<String, Vec<&str>> {
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
