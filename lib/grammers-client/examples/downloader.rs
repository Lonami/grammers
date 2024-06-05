//! Example to download all messages and media from a chat.
//!
//! The `TG_ID` and `TG_HASH` environment variables must be set (learn how to do it for
//! [Windows](https://ss64.com/nt/set.html) or [Linux](https://ss64.com/bash/export.html))
//! to Telegram's API ID and API hash respectively.
//!
//! Then, run it as:
//!
//! ```sh
//! cargo run --example downloader -- CHAT_NAME
//! ```
//!
//! Messages will be printed to stdout, and media will be saved in the `target/` folder locally, named
//! message-[MSG_ID].[EXT]
//!
use std::io::{BufRead, Write};
use std::path::Path;
use std::{env, io};

use grammers_client::{Client, Config, SignInError};
use mime::Mime;
use mime_guess::mime;
use simple_logger::SimpleLogger;
use tokio::runtime;

use grammers_client::types::Media::{Contact, Document, Photo, Sticker};
use grammers_client::types::*;
use grammers_session::Session;

type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

const SESSION_FILE: &str = "downloader.session";

async fn async_main() -> Result<()> {
    SimpleLogger::new()
        .with_level(log::LevelFilter::Info)
        .init()
        .unwrap();

    let api_id = std::env::var("TG_ID")?.parse().expect("TG_ID invalid");
    let api_hash = std::env::var("TG_HASH")?.to_string();
    let chat_name = env::args().nth(1).expect("chat name missing");

    println!("Connecting to Telegram...");
    let client = Client::connect(Config {
        session: Session::load_file_or_create(SESSION_FILE)?,
        api_id,
        api_hash: api_hash.clone(),
        params: Default::default(),
    })
    .await?;
    println!("Connected!");

    // If we can't save the session, sign out once we're done.
    let mut sign_out = false;

    if !client.is_authorized().await? {
        println!("Signing in...");
        let phone = prompt("Enter your phone number (international format): ")?;
        let token = client.request_login_code(&phone).await?;
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
    }

    let maybe_chat = client.resolve_username(chat_name.as_str()).await?;

    let chat = maybe_chat.unwrap_or_else(|| panic!("Chat {} could not be found", chat_name));

    let mut messages = client.iter_messages(&chat);

    println!(
        "Chat {} has {} total messages.",
        chat_name,
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
                .download_media(&Downloadable::Media(media), &Path::new(dest.as_str()))
                .await
                .expect("Error downloading message");
        }
    }

    println!("Downloaded {} messages", counter);

    if sign_out {
        // TODO revisit examples and get rid of "handle references" (also, this panics)
        drop(client.sign_out_disconnect().await);
    }

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
        Photo(_) => ".jpg".to_string(),
        Sticker(sticker) => get_mime_extension(sticker.document.mime_type()),
        Document(document) => get_mime_extension(document.mime_type()),
        Contact(_) => ".vcf".to_string(),
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
