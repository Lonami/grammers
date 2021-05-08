// Copyright 2020 - developers of the `grammers` project.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// https://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or https://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.
use std::fs::File;
use std::io::Read;
use toml::Value;

#[test]
fn check_deps_documented() {
    let mut listed_deps = {
        let mut deps = std::collections::HashSet::new();
        let mut file = File::open("Cargo.toml").expect("Cargo.toml must exist");
        let mut toml = String::new();
        file.read_to_string(&mut toml)
            .expect("Cargo.toml should not fail to be read");

        match toml.parse::<toml::Value>() {
            Ok(Value::Table(mut map)) => {
                for &key in ["dependencies", "build-dependencies", "dev-dependencies"].iter() {
                    if let Some(Value::Table(build)) = map.remove(key) {
                        for (dep, _) in build {
                            deps.insert(dep);
                        }
                    }
                }
            }
            _ => unreachable!("Cargo.toml should not be malformed"),
        }

        deps.into_iter().collect::<Vec<_>>()
    };
    listed_deps.sort();

    let mut documented_deps = {
        let mut file = File::open("DEPS.md").expect("DEPS.md must exist");
        let mut markdown = String::new();
        file.read_to_string(&mut markdown)
            .expect("DEPS.md should not fail to be read");

        markdown
            .lines()
            .filter_map(|line| {
                if line.starts_with("## ") {
                    Some(line[3..].to_string())
                } else {
                    None
                }
            })
            .collect::<Vec<_>>()
    };
    documented_deps.sort();

    let mut undocumented_deps = Vec::new();
    while let Some(dep) = listed_deps.pop() {
        if let Some(idx) = documented_deps.iter().position(|d| &dep == d) {
            documented_deps.remove(idx);
        } else {
            undocumented_deps.push(dep);
        }
    }

    assert!(
        undocumented_deps.is_empty(),
        "some Cargo.toml dependencies are not in DEPS.md: {:?}",
        undocumented_deps
    );
    assert!(
        documented_deps.is_empty(),
        "DEPS.md lists dependencies no longer present in Cargo.toml: {:?}",
        documented_deps
    );
}
