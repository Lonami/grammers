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

use std::sync::Arc;
use std::{env, time::Duration};

use grammers_client::Client;
use grammers_client::client::UpdatesConfiguration;
use grammers_client::update::Update;
use grammers_mtsender::SenderPool;
use grammers_session::storages::SqliteSession;
use simple_logger::SimpleLogger;
use tokio::task::JoinSet;
use tokio::{runtime, time::sleep};

type Result = std::result::Result<(), Box<dyn std::error::Error>>;

const SESSION_FILE: &str = "echo.session";

async fn handle_update(client: Client, update: Update) {
    match update {
        Update::NewMessage(message) if !message.outgoing() => {
            let peer = message.peer().unwrap();
            println!(
                "Responding to {}",
                peer.name().unwrap_or(&format!("id {}", message.peer_id()))
            );
            if message.text() == "slow" {
                sleep(Duration::from_secs(5)).await;
            }
            if let Err(e) = client
                .send_message(peer.to_ref().await.unwrap(), message.text())
                .await
            {
                println!("Failed to respond! {e}");
            };
        }
        _ => {}
    }
}

async fn async_main() -> Result {
    SimpleLogger::new()
        .with_level(log::LevelFilter::Debug)
        .init()
        .unwrap();

    let api_id = env!("TG_ID").parse().expect("TG_ID invalid");
    let token = env::args().nth(1).expect("token missing");

    let session = Arc::new(SqliteSession::open(SESSION_FILE).await?);

    let SenderPool {
        runner,
        updates,
        handle,
    } = SenderPool::new(Arc::clone(&session), api_id);
    let client = Client::new(handle.clone());
    let pool_task = tokio::spawn(runner.run());

    if !client.is_authorized().await? {
        println!("Signing in...");
        client.bot_sign_in(&token, env!("TG_HASH")).await?;
        println!("Signed in!");
    }

    println!("Waiting for messages...");

    // This example spawns a task to handle each update.
    // To guarantee that all handlers run to completion, they're stored in this set.
    // You can use `task::spawn` if you don't care about dropping unfinished handlers midway.
    let mut handler_tasks = JoinSet::new();
    let mut updates = client
        .stream_updates(
            updates,
            UpdatesConfiguration {
                catch_up: true,
                ..Default::default()
            },
        )
        .await;
    loop {
        // Empty finished handlers (you could look at their return value here too.)
        while let Some(_) = handler_tasks.try_join_next() {}

        // This code uses `select` on Ctrl+C to gracefully stop the client and have a chance to
        // save the session. You could have fancier logic to save the session if you wanted to
        // (or even save it on every update). Or you could also ignore Ctrl+C and just use
        // `let update = client.next_update().await?`.
        tokio::select! {
            _ = tokio::signal::ctrl_c() => break,
            update = updates.next() => {
                let update = update?;
                let handle = client.clone();
                handler_tasks.spawn(handle_update(handle, update));
            }
        }
    }

    println!("Saving session file...");
    updates.sync_update_state().await; // you usually want this before closing the session

    // Pool's `run()` won't finish until all handles are dropped or quit is called.
    // Here there are at least three handles alive: `handle`, `client` and `updates`
    // which contains a `client`. Any ongoing `handle_update` handlers have one client too.
    // In this case, it's easier to call `handle.quit()` to close them all.
    //
    // You don't need to explicitly close the connection, but this is a way to do it gracefully.
    // This also gives a chance to the handlers to finish their work by handling the `Dropped`
    // error from any pending method calls (RPC invocations).
    //
    // You can try this graceful shutdown by sending a message saying "slow" and then pressing Ctrl+C.
    println!("Gracefully closing connection to notify all pending handlers...");
    handle.quit();
    let _ = pool_task.await;

    // Give a chance to all on-going handlers to finish.
    println!("Waiting for any slow handlers to finish...");
    while let Some(_) = handler_tasks.join_next().await {}

    Ok(())
}

fn main() -> Result {
    runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap()
        .block_on(async_main())
}
