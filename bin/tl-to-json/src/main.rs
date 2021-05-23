// Copyright 2020 - developers of the `grammers` project.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// https://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or https://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

//! Read every `.tl` file given as input parameter, and output its `json`
//! variant next to it.
//!
//! If the file is "-", it is read from standard input instead.
use grammers_tl_parser::{parse_tl_file, tl};
use std::env;
use std::fs::File;
use std::io::{self, BufWriter, Read};
use std::path::PathBuf;

const STDIN_NAME: &str = "-";

#[derive(serde::Serialize)]
struct Schema {
    constructors: Vec<Constructor>,
    methods: Vec<Method>,
}

#[derive(serde::Serialize)]
struct Constructor {
    id: String,
    predicate: String,
    params: Vec<Parameter>,
    r#type: String,
}

#[derive(serde::Serialize)]
struct Method {
    id: String,
    method: String,
    params: Vec<Parameter>,
    r#type: String,
}

#[derive(serde::Serialize)]
struct Parameter {
    name: String,
    r#type: String,
}

fn adapt_id(id: u32) -> String {
    (id as i32).to_string()
}

fn full_name(ns: &[String], name: &str) -> String {
    let mut result = String::new();
    ns.iter().for_each(|ns| {
        result.push_str(ns);
        result.push('.');
    });
    result.push_str(name);
    result
}

fn adapt_param(ty: &tl::Parameter) -> Parameter {
    Parameter {
        name: ty.name.clone(),
        r#type: ty.ty.to_string(),
    }
}

fn main() -> std::io::Result<()> {
    // load_tl("tl/api.tl")?);
    let mut tl = String::new();
    for fin in env::args().skip(1) {
        if fin == STDIN_NAME {
            io::stdin().read_to_string(&mut tl)?;
        } else {
            File::open(&fin)?.read_to_string(&mut tl)?;
        }

        let mut schema = Schema {
            constructors: Vec::new(),
            methods: Vec::new(),
        };
        parse_tl_file(&tl)
            .into_iter()
            .filter_map(Result::ok)
            .for_each(|def| match def.category {
                tl::Category::Types => schema.constructors.push(Constructor {
                    id: adapt_id(def.id),
                    predicate: full_name(&def.namespace, &def.name),
                    params: def.params.iter().map(adapt_param).collect(),
                    r#type: def.ty.to_string(),
                }),
                tl::Category::Functions => schema.methods.push(Method {
                    id: adapt_id(def.id),
                    method: full_name(&def.namespace, &def.name),
                    params: def.params.iter().map(adapt_param).collect(),
                    r#type: def.ty.to_string(),
                }),
            });

        if fin == STDIN_NAME {
            serde_json::to_writer(io::stdout(), &schema)?;
        } else {
            let mut fout = PathBuf::from(&fin);
            fout.set_extension("json");
            serde_json::to_writer(BufWriter::new(File::create(fout)?), &schema)?;
        }
    }

    Ok(())
}
