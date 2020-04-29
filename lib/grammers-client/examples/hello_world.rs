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

use async_std::task;
use grammers_client::{AuthorizationError, Client, Config};
use grammers_session::Session;
use log;
use simple_logger;
use std::env;

async fn async_main() -> Result<(), AuthorizationError> {
    simple_logger::init_with_level(log::Level::Debug).expect("failed to setup logging");

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

    println!("Connecting to Telegram...");
    let mut client = Client::connect(Config {
        session: Session::load_or_create("hello.session")?,
        api_id,
        api_hash: api_hash.clone(),
        params: Default::default(),
    })
    .await?;
    println!("Connected!");

    if !client.is_authorized().await? {
        println!("Signing in...");
        client.bot_sign_in(&token, api_id, &api_hash).await?;
        println!("Signed in!");
    }

    println!("Sending message...");
    let user = client.input_peer_for_username(&username).await?;
    client.send_message(user, message.into()).await?;
    println!("Message sent!");

    Ok(())
}

fn main() -> Result<(), AuthorizationError> {
    task::block_on(async_main())
}
