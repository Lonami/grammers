//! This example fetches all dialogs and prints them to the console.
//!
//! ```sh
//! cargo run --example print_dialogs -- API_ID API_HASH
//! ```

use std::env;
use std::io::{self, Write};

use grammers_client::iterators::RPCIterator;
use grammers_client::{AuthorizationError, Client, Dialogs, InvocationError};
use grammers_session::TextSession;

fn ask_input(message: &str) -> Result<String, InvocationError> {
    let mut input = String::new();
    print!("{}", message);
    io::stdout().flush()?;
    io::stdin().read_line(&mut input)?;
    Ok(input)
}

fn main() -> Result<(), AuthorizationError> {
    let mut args = env::args();

    let _path = args.next();
    let api_id = args
        .next()
        .expect("api_id missing")
        .parse()
        .expect("api_id invalid");
    let api_hash = args.next().expect("api_hash missing");

    let session = Box::new(if let Ok(session) = TextSession::load(&"user.session") {
        session
    } else {
        TextSession::create("user.session")?
    });

    println!("Connecting to Telegram...");
    let mut client = Client::with_session(session)?;
    println!("Connected!");

    if !client.is_authorized()? {
        let phone = ask_input("Enter your phone (international format): ")?;
        client.request_login_code(&phone, api_id, &api_hash)?;

        let code = ask_input("Enter the code you received: ")?;
        client.sign_in(&code).expect("failed to login");
    }

    let mut iter = Dialogs::iter();
    while let Some(dialog) = iter.next(&mut client)? {
        println!("[{:>10}] {}", dialog.entity.id(), dialog.entity.display());
    }

    Ok(())
}
