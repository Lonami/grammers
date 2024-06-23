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
    parse_tl_file(contents).map(|d| d.unwrap()).collect()
}

fn gen_rust_code(definitions: &[Definition]) -> io::Result<String> {
    let mut file = Vec::new();
    generate_rust_code(
        &mut file,
        definitions,
        LAYER,
        &Config {
            gen_name_for_id: false,
            deserializable_functions: true,
            impl_debug: true,
            impl_from_enum: true,
            impl_from_type: true,
        },
    )?;
    Ok(String::from_utf8(file).unwrap())
}

#[test]
fn generic_functions_use_generic_parameters() -> io::Result<()> {
    let definitions = get_definitions(
        "
        ---functions---
        invokeWithLayer#da9b0d0d {X:Type} layer:int query:!X = X;
    ",
    );
    let result = gen_rust_code(&definitions)?;
    eprintln!("{result}");
    assert!(result.contains("pub struct InvokeWithLayer<X>"));
    assert!(result.contains("pub query: X,"));
    assert!(result.contains("impl<X> crate::Identifiable for InvokeWithLayer<X>"));
    assert!(
        result.contains("impl<X: crate::Serializable> crate::Serializable for InvokeWithLayer<X>")
    );
    assert!(result
        .contains("impl<X: crate::Deserializable> crate::Deserializable for InvokeWithLayer<X>"));
    assert!(result.contains("impl<X: crate::RemoteCall> crate::RemoteCall for InvokeWithLayer<X>"));
    assert!(result.contains("type Return = X::Return;"));
    Ok(())
}

#[test]
fn recursive_types_direct_boxed() -> io::Result<()> {
    let definitions = get_definitions(
        "
        textBold#6724abc4 text:RichText = RichText;
    ",
    );
    let result = gen_rust_code(&definitions)?;
    eprintln!("{result}");
    assert!(result.contains("TextBold(Box<crate::types::TextBold>)"));
    assert!(result.contains("RichText::TextBold(Box::new("));
    assert!(result.contains("Self::TextBold(Box::new("));
    Ok(())
}

#[test]
fn recursive_types_indirect_boxed() -> io::Result<()> {
    let definitions = get_definitions(
        "
        messageExtendedMedia#ee479c64 media:MessageMedia = MessageExtendedMedia;
        messageMediaInvoice#f6a548d3 flags:# extended_media:flags.4?MessageExtendedMedia = MessageMedia;
    ",
    );
    let result = gen_rust_code(&definitions)?;
    eprintln!("{result}");
    assert!(result.contains("Media(Box<crate::types::MessageExtendedMedia>),"));
    assert!(result.contains("Box::new(crate::types::MessageExtendedMedia::deserialize("));
    assert!(result.contains("MessageExtendedMedia::Media(Box::new("));
    assert!(result.contains("Invoice(Box<crate::types::MessageMediaInvoice>),"));
    assert!(result.contains("Box::new(crate::types::MessageMediaInvoice::deserialize("));
    assert!(result.contains("MessageMedia::Invoice(Box::new("));
    Ok(())
}

#[test]
fn recursive_types_indirect_no_hang() -> io::Result<()> {
    let definitions = get_definitions(
        "
        inputUserFromMessage#1da448e2 peer:InputPeer msg_id:int user_id:long = InputUser;
        inputPeerUserFromMessage#a87b0a1c peer:InputPeer msg_id:int user_id:long = InputPeer;
    ",
    );
    let _ = gen_rust_code(&definitions)?;
    Ok(())
}

#[test]
fn recursive_types_vec_indirect_not_boxed() -> io::Result<()> {
    let definitions = get_definitions(
        "
        jsonObjectValue#c0de1bd9 key:string value:JSONValue = JSONObjectValue;

        jsonArray#f7444763 value:Vector<JSONValue> = JSONValue;
        jsonObject#99c1d49d value:Vector<JSONObjectValue> = JSONValue;
    ",
    );
    let result = gen_rust_code(&definitions)?;
    eprintln!("{result}");
    assert!(result.contains("JsonObjectValue(crate::types::JsonObjectValue)"));
    assert!(result.contains("JsonArray(crate::types::JsonArray)"));
    assert!(result.contains("JsonObject(crate::types::JsonObject)"));
    Ok(())
}
