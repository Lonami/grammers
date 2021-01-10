//! Example to echo user text messages. Updates are handled concurrently.
//!
//! The `TG_ID` and `TG_HASH` environment variables must be set (learn how to do it for
//! [Windows](https://ss64.com/nt/set.html) or [Linux](https://ss64.com/bash/export.html))
//! to Telegram's API ID and API hash respectively.
//!
//! Then, run it as:
//!
//! ```sh
//! cargo run --example echo -- BOT_TOKEN
//! ```

use grammers_client::{Client, ClientHandle, Config, Update, UpdateIter};
use grammers_session::FileSession;
use log;
use simple_logger::SimpleLogger;
use std::env;
use tokio::{runtime, task};

type Result = std::result::Result<(), Box<dyn std::error::Error>>;

async fn handle_update(mut client: ClientHandle, updates: UpdateIter) -> Result {
    for update in updates {
        match update {
            Update::NewMessage(message) if !message.outgoing() => {
                let chat = message.chat();
                println!("Responding to {}", chat.name());
                client.send_message(&chat, message.text().into()).await?;
            }
            _ => {}
        }
    }

    Ok(())
}

async fn async_main() -> Result {
    SimpleLogger::new()
        .with_level(log::LevelFilter::Debug)
        .init()
        .unwrap();

    let api_id = env!("TG_ID").parse().expect("TG_ID invalid");
    let api_hash = env!("TG_HASH").to_string();
    let token = env::args().skip(1).next().expect("token missing");

    println!("Connecting to Telegram...");
    let mut client = Client::connect(Config {
        session: FileSession::load_or_create("echo.session")?,
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

    println!("Waiting for messages...");
    while let Some(updates) = client.next_updates().await? {
        let handle = client.handle();
        task::spawn(async move {
            match handle_update(handle, updates).await {
                Ok(_) => {}
                Err(e) => eprintln!("Error handling updates!: {}", e),
            }
        });
    }

    Ok(())
}

fn main() -> Result {
    runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap()
        .block_on(async_main())
}
