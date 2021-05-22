// Copyright 2020 - developers of the `grammers` project.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// https://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or https://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.
use grammers_tl_gen::{generate_rust_code, Config};
use grammers_tl_parser::parse_tl_file;
use grammers_tl_parser::tl::Definition;
use std::io;

const LAYER: i32 = 0;

fn get_definitions(contents: &str) -> Vec<Definition> {
    parse_tl_file(&contents)
        .into_iter()
        .map(|d| d.unwrap())
        .collect()
}

#[test]
fn generic_functions_use_generic_parameters() -> io::Result<()> {
    let definitions = get_definitions(
        "
        ---functions---
        invokeWithLayer#da9b0d0d {X:Type} layer:int query:!X = X;
    ",
    );
    let mut file = Vec::new();
    generate_rust_code(
        &mut file,
        &definitions,
        LAYER,
        &Config {
            gen_name_for_id: false,
            deserializable_functions: true,
            impl_debug: true,
            impl_from_enum: true,
            impl_from_type: true,
        },
    )?;
    let result = String::from_utf8(file).unwrap();
    eprintln!("{}", result);
    assert!(result.contains("InvokeWithLayer<X: crate::RemoteCall>"));
    assert!(result.contains("pub query: X,"));
    assert!(
        result.contains("impl<X: crate::RemoteCall> crate::Identifiable for InvokeWithLayer<X>")
    );
    assert!(
        result.contains("impl<X: crate::RemoteCall> crate::Serializable for InvokeWithLayer<X>")
    );
    assert!(
        result.contains("impl<X: crate::RemoteCall> crate::Deserializable for InvokeWithLayer<X>")
    );
    assert!(result.contains("impl<X: crate::RemoteCall> crate::RemoteCall for InvokeWithLayer<X>"));
    assert!(result.contains("type Return = X::Return;"));
    Ok(())
}
