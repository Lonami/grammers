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

use futures_util::future::{Either, select};
use grammers_client::{Client, Update};
use grammers_mtsender::SenderPool;
use grammers_session::storages::TlSession;
use simple_logger::SimpleLogger;
use std::env;
use std::pin::pin;
use std::sync::Arc;
use tokio::{runtime, task};

type Result = std::result::Result<(), Box<dyn std::error::Error>>;

const SESSION_FILE: &str = "echo.session";

async fn handle_update(client: Client, update: Update) -> Result {
    match update {
        Update::NewMessage(message) if !message.outgoing() => {
            let chat = message.chat();
            println!(
                "Responding to {}",
                chat.name().unwrap_or(&format!("id {}", chat.id()))
            );
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

    let api_id = env!("TG_ID").parse().expect("TG_ID invalid");
    let token = env::args().nth(1).expect("token missing");

    let session = Arc::new(TlSession::load_file_or_create(SESSION_FILE)?);

    let pool = SenderPool::new(Arc::clone(&session), api_id);
    let client = Client::new(&pool);
    let SenderPool {
        runner,
        handle,
        updates,
    } = pool;
    let pool_task = tokio::spawn(runner.run());

    println!("Connecting to Telegram...");
    println!("Connected!");

    if !client.is_authorized().await? {
        println!("Signing in...");
        client.bot_sign_in(&token, env!("TG_HASH")).await?;
        session.save_to_file(SESSION_FILE)?;
        println!("Signed in!");
    }

    println!("Waiting for messages...");

    // This code uses `select` on Ctrl+C to gracefully stop the client and have a chance to
    // save the session. You could have fancier logic to save the session if you wanted to
    // (or even save it on every update). Or you could also ignore Ctrl+C and just use
    // `let update = client.next_update().await?`.
    //
    // Using `tokio::select!` would be a lot cleaner but add a heavy dependency,
    // so a manual `select` is used instead by pinning async blocks by hand.
    let mut updates = client.stream_updates(updates);
    loop {
        let exit = pin!(async { tokio::signal::ctrl_c().await });
        let upd = pin!(async { updates.next().await });

        let update = match select(exit, upd).await {
            Either::Left(_) => break,
            Either::Right((u, _)) => u?,
        };

        let handle = client.clone();
        task::spawn(async move {
            match handle_update(handle, update).await {
                Ok(_) => {}
                Err(e) => eprintln!("Error handling updates!: {e}"),
            }
        });
    }

    println!("Saving session file and exiting...");
    session.save_to_file(SESSION_FILE)?;

    handle.quit();
    let _ = pool_task.await;

    Ok(())
}

fn main() -> Result {
    runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap()
        .block_on(async_main())
}
