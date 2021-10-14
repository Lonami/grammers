//! Example to print the ID and title of all the dialogs.
//!
//! The `TG_ID` and `TG_HASH` environment variables must be set (learn how to do it for
//! [Windows](https://ss64.com/nt/set.html) or [Linux](https://ss64.com/bash/export.html))
//! to Telegram's API ID and API hash respectively.
//!
//! Then, run it as:
//!
//! ```sh
//! cargo run --example dialogs
//! ```

use grammers_client::Client;
use log;
use simple_logger::SimpleLogger;
use std::env;
use tokio::{runtime, task};

type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

const SESSION_FILE: &str = "dialogs.session";

async fn async_main() -> Result<()> {
    SimpleLogger::new()
        .with_level(log::LevelFilter::Debug)
        .init()
        .unwrap();

    let api_id = env!("TG_ID").parse().expect("TG_ID invalid");
    let api_hash = env!("TG_HASH").to_string();

    let (mut client, _authorized) = Client::builder(api_id, &api_hash)
        .interactive(true)
        .show_password_hint(true)
        .session_file(SESSION_FILE)?
        .connect()
        .await?;

    let mut sign_out = false;
    println!("Signed in!");
    match client.session().save_to_file(SESSION_FILE) {
        Ok(_) => {}
        Err(e) => {
            println!(
                "NOTE: failed to save the session, will sign out when done: {}",
                e
            );
            sign_out = true;
        }
    }

    // Obtain a `ClientHandle` to perform remote calls while `Client` drives the connection.
    //
    // This handle can be `clone()`'d around and freely moved into other tasks, so you can invoke
    // methods concurrently if you need to. While you do this, the single owned `client` is the
    // one that communicates with the network.
    //
    // The design's annoying to use for trivial sequential tasks, but is otherwise scalable.
    let mut client_handle = client.clone();
    let network_handle = task::spawn(async move { client.run_until_disconnected().await });

    let mut dialogs = client_handle.iter_dialogs();

    println!("Showing up to {} dialogs:", dialogs.total().await?);
    while let Some(dialog) = dialogs.next().await? {
        let chat = dialog.chat();
        println!("- {: >10} {}", chat.id(), chat.name());
    }

    if sign_out {
        // TODO revisit examples and get rid of "handle references" (also, this panics)
        drop(client_handle.sign_out_disconnect().await);
    }

    network_handle.await??;
    Ok(())
}

fn main() -> Result<()> {
    runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap()
        .block_on(async_main())
}
