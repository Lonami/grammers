// Copyright 2020 - developers of the `grammers` project.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// https://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or https://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.
use grammers_tl_gen::{generate_rust_code, Config};
use grammers_tl_parser::parse_tl_file;
use std::env;
use std::fs::File;
use std::io::{BufWriter, Write};
use std::path::Path;

const CURRENT_VERSION: i32 = 3;

fn main() -> std::io::Result<()> {
    let mut file = BufWriter::new(File::create(
        Path::new(&env::var("OUT_DIR").unwrap()).join("generated.rs"),
    )?);

    // Using boxed variants in the definitions so that deserialization fails if any constructor ID changes.
    let definitions = parse_tl_file(
        r#"
        dataCenter flags:# id:int ipv4:flags.0?int ipv6:flags.1?int128 port:int auth:flags.2?bytes = DataCenter;
        dataCenterWs flags:# id:int url:string auth:flags.0?bytes = DataCenter;
        user id:long dc:int bot:Bool = User;
        channelState channel_id:long pts:int = ChannelState;
        updateState pts:int qts:int date:int seq:int channels:Vector<ChannelState> = UpdateState;
        session flags:# dcs:Vector<DataCenter> user:flags.0?User state:flags.1?UpdateState = Session;
        "#,
    )
    .map(Result::unwrap)
    .collect::<Vec<_>>();

    let config = Config {
        ..Default::default()
    };

    generate_rust_code(&mut file, &definitions, CURRENT_VERSION, &config)?;
    file.flush()?;

    Ok(())
}
