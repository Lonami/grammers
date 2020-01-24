use grammers_tl_parser::{parse_tl_file, Definition, ParseError};
use std::fs::File;
use std::io::prelude::*;

fn load_tl(file: &str) -> std::io::Result<Vec<Result<Definition, ParseError>>> {
    let mut file = File::open(file)?;
    let mut contents = String::new();
    file.read_to_string(&mut contents)?;
    Ok(parse_tl_file(&contents))
}

fn main() -> std::io::Result<()> {
    let api = load_tl("tl/api.tl")?;
    let mtproto = load_tl("tl/mtproto.tl")?;

    // TODO Generate rust code with these definitions

    // See target/*/build/*/stderr
    for definition in api {
        dbg!(definition);
    }
    for definition in mtproto {
        dbg!(definition);
    }

    Ok(())
}
