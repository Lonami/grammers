//! A hello world example. Runnable as:
//!
//! ```sh
//! cargo run --example hello_world -- API_ID API_HASH BOT_TOKEN USERNAME MESSAGE
//! ```
//!
//! For example, to send 'Hello, world!' to the person '@username':
//!
//! ```sh
//! cargo run --example hello_world -- 123 1234abc 123:abc username 'Hello, world!'
//! ```

use grammers_client::Client;
use grammers_session::TextSession;
use std::env;
use std::io::Result;

fn main() -> Result<()> {
    let mut args = env::args();

    let _path = args.next();
    let api_id = args
        .next()
        .expect("api_id missing")
        .parse()
        .expect("api_id invalid");
    let api_hash = args.next().expect("api_hash missing");
    let token = args.next().expect("token missing");
    let username = args.next().expect("username missing");
    let message = args.next().expect("message missing");

    // Try loading a previous session, or create a new one otherwise.
    let session = Box::new(if let Ok(session) = TextSession::load(&"hello.session") {
        session
    } else {
        TextSession::create("hello.session")?
    });

    println!("Connecting to Telegram...");
    let mut client = Client::with_session(session)?;
    println!("Connected!");

    println!("Initializing connection...");
    client.init_connection()?;
    println!("Connection initialized!");

    println!("Signing in...");
    client.bot_sign_in(&token, api_id, &api_hash)?;
    println!("Signed in!");

    println!("Sending message...");
    client.send_message(&username[..], &message)?;
    println!("Message sent!");

    Ok(())
}
