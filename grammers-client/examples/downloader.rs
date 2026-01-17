//! Example to download all messages and media from a peer.
//!
//! The `TG_ID` and `TG_HASH` environment variables must be set (learn how to do it for
//! [Windows](https://ss64.com/nt/set.html) or [Linux](https://ss64.com/bash/export.html))
//! to Telegram's API ID and API hash respectively.
//!
//! Then, run it as:
//!
//! ```sh
//! cargo run --example downloader -- PEER_NAME
//! ```
//!
//! Messages will be printed to stdout, and media will be saved in the `target/` folder locally, named
//! message-[MSG_ID].[EXT]

use std::io::{BufRead, Write};
use std::path::Path;
use std::sync::Arc;
use std::{env, io};

use grammers_client::media::Media;
use grammers_client::{Client, SignInError};
use grammers_mtsender::SenderPool;
use grammers_session::storages::SqliteSession;
use mime::Mime;
use mime_guess::mime;
use simple_logger::SimpleLogger;
use tokio::runtime;

type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

const SESSION_FILE: &str = "downloader.session";

async fn async_main() -> Result<()> {
    SimpleLogger::new()
        .with_level(log::LevelFilter::Info)
        .init()
        .unwrap();

    let api_id = env!("TG_ID").parse().expect("TG_ID invalid");
    let peer_name = env::args().nth(1).expect("peer name missing");

    let session = Arc::new(SqliteSession::open(SESSION_FILE).await?);

    let SenderPool { runner, handle, .. } = SenderPool::new(Arc::clone(&session), api_id);
    let client = Client::new(handle);
    let _ = tokio::spawn(runner.run());

    if !client.is_authorized().await? {
        println!("Signing in...");
        let phone = prompt("Enter your phone number (international format): ")?;
        let token = client.request_login_code(&phone, env!("TG_HASH")).await?;
        let code = prompt("Enter the code you received: ")?;
        let signed_in = client.sign_in(&token, &code).await;
        match signed_in {
            Err(SignInError::PasswordRequired(password_token)) => {
                // Note: this `prompt` method will echo the password in the console.
                //       Real code might want to use a better way to handle this.
                let hint = password_token.hint().unwrap();
                let prompt_message = format!("Enter the password (hint {}): ", &hint);
                let password = prompt(prompt_message.as_str())?;

                client
                    .check_password(password_token, password.trim())
                    .await?;
            }
            Ok(_) => (),
            Err(e) => panic!("{}", e),
        };
        println!("Signed in!");
    }

    let maybe_peer = client
        .resolve_username(peer_name.as_str())
        .await?
        .ok_or("no peer with username")?
        .to_ref()
        .await;

    let peer = maybe_peer.unwrap_or_else(|| panic!("Peer {peer_name} could not be found"));

    let mut messages = client.iter_messages(peer);

    println!(
        "Peer {} has {} total messages.",
        peer_name,
        messages.total().await.unwrap()
    );

    let mut counter = 0;

    while let Some(msg) = messages.next().await? {
        counter += 1;
        println!("Message {}:{}", msg.id(), msg.text());
        if let Some(media) = msg.media() {
            let dest = format!(
                "target/message-{}{}",
                &msg.id().to_string(),
                get_file_extension(&media)
            );
            client
                .download_media(&media, &Path::new(dest.as_str()))
                .await
                .expect("Error downloading message");
        }
    }

    println!("Downloaded {counter} messages");

    // `runner.run()`'s task will be dropped (and disconnect occur) once the runtime exits.
    Ok(())
}

fn main() -> Result<()> {
    runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap()
        .block_on(async_main())
}

fn get_file_extension(media: &Media) -> String {
    match media {
        Media::Photo(_) => ".jpg".to_string(),
        Media::Sticker(sticker) => get_mime_extension(sticker.document.mime_type()),
        Media::Document(document) => get_mime_extension(document.mime_type()),
        Media::Contact(_) => ".vcf".to_string(),
        _ => String::new(),
    }
}

fn get_mime_extension(mime_type: Option<&str>) -> String {
    mime_type
        .map(|m| {
            let mime: Mime = m.parse().unwrap();
            format!(".{}", mime.subtype())
        })
        .unwrap_or_default()
}

fn prompt(message: &str) -> Result<String> {
    let stdout = io::stdout();
    let mut stdout = stdout.lock();
    stdout.write_all(message.as_bytes())?;
    stdout.flush()?;

    let stdin = io::stdin();
    let mut stdin = stdin.lock();

    let mut line = String::new();
    stdin.read_line(&mut line)?;
    Ok(line)
}
