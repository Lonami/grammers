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

use futures_util::future::{select, Either};
use grammers_client::{Client, Config, InitParams, Update};
use grammers_session::Session;
use simple_logger::SimpleLogger;
use std::env;
use std::pin::pin;
use tokio::{runtime, task};

type Result = std::result::Result<(), Box<dyn std::error::Error>>;

const SESSION_FILE: &str = "echo.session";

async fn handle_update(client: Client, update: Update) -> Result {
    match update {
        Update::NewMessage(message) if !message.outgoing() => {
            let chat = message.chat();
            println!("Responding to {}", chat.name());
            client.send_message(&chat, message.text()).await?;
        }
        _ => {}
    }

    Ok(())
}

async fn async_main() -> Result {
    SimpleLogger::new()
        .with_level(log::LevelFilter::Debug)
        .init()
        .unwrap();

    let api_id = std::env::var("TG_ID")?.parse().expect("TG_ID invalid");
    let api_hash = std::env::var("TG_HASH")?.to_string();
    let token = env::args().nth(1).expect("token missing");

    println!("Connecting to Telegram...");
    let client = Client::connect(Config {
        session: Session::load_file_or_create(SESSION_FILE)?,
        api_id,
        api_hash: api_hash.clone(),
        params: InitParams {
            // Fetch the updates we missed while we were offline
            catch_up: true,
            ..Default::default()
        },
    })
    .await?;
    println!("Connected!");

    if !client.is_authorized().await? {
        println!("Signing in...");
        client.bot_sign_in(&token).await?;
        client.session().save_to_file(SESSION_FILE)?;
        println!("Signed in!");
    }

    println!("Waiting for messages...");

    // This code uses `select` on Ctrl+C to gracefully stop the client and have a chance to
    // save the session. You could have fancier logic to save the session if you wanted to
    // (or even save it on every update). Or you could also ignore Ctrl+C and just use
    // `while let Some(updates) =  client.next_updates().await?`.
    //
    // Using `tokio::select!` would be a lot cleaner but add a heavy dependency,
    // so a manual `select` is used instead by pinning async blocks by hand.
    loop {
        let update = {
            let exit = pin!(async { tokio::signal::ctrl_c().await });
            let upd = pin!(async { client.next_update().await });

            match select(exit, upd).await {
                Either::Left(_) => None,
                Either::Right((u, _)) => Some(u),
            }
        };

        let update = match update {
            None | Some(Ok(None)) => break,
            Some(u) => u?.unwrap(),
        };

        let handle = client.clone();
        task::spawn(async move {
            match handle_update(handle, update).await {
                Ok(_) => {}
                Err(e) => eprintln!("Error handling updates!: {}", e),
            }
        });
    }

    println!("Saving session file and exiting...");
    client.session().save_to_file(SESSION_FILE)?;
    Ok(())
}

fn main() -> Result {
    runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap()
        .block_on(async_main())
}
