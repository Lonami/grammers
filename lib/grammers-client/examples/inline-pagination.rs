//! Example to showcase how to achieve pagination with inline buttons.
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
//!
//! In order to achieve pagination, the button data must contain enough information for the code
//! to determine which "offset" should it use when querying additional data.
//!
//! For this example, the button contains the last two values of the fibonacci sequence where it
//! last left off, separated by a comma. This way, the code can resume.
//!
//! If there is no comma, then there is a single number, which we use to know what was clicked.
//!
//! If it's the special string "done", then we know we've reached the end (there is a limit to
//! how much data a button's payload can contain, and to keep it simple, we're storing it inline
//! in decimal, so the numbers can't get too large).

use grammers_client::{button, reply_markup, Client, Config, InputMessage, Update};
use grammers_session::Session;
use log;
use simple_logger::SimpleLogger;
use std::env;
use tokio::{runtime, task};

type Result = std::result::Result<(), Box<dyn std::error::Error>>;

const SESSION_FILE: &str = "inline-pagination.session";

const NUMBERS_PER_PAGE: usize = 4;

// https://core.telegram.org/bots/api#inlinekeyboardbutton
const MAX_PAYLOAD_DATA_LEN: usize = 64;

/// Generate the inline keyboard reply markup with a few more numbers from the sequence.
fn fib_markup(mut a: u128, mut b: u128) -> reply_markup::Inline {
    let mut rows = Vec::with_capacity(NUMBERS_PER_PAGE + 1);
    for _ in 0..NUMBERS_PER_PAGE {
        let text = a.to_string();
        rows.push(vec![button::inline(&text, text.as_bytes())]);

        let bb = b;
        b = a + b;
        a = bb;
    }

    let next = format!("{},{}", a, b);
    if next.len() > MAX_PAYLOAD_DATA_LEN {
        rows.push(vec![button::inline("I'm satisfied!!", b"done".to_vec())]);
    } else {
        rows.push(vec![
            button::inline("Restart!", b"0,1".to_vec()),
            button::inline("More!", format!("{},{}", a, b).into_bytes()),
        ]);
    }
    reply_markup::inline(rows)
}

async fn handle_update(_client: Client, update: Update) -> Result {
    match update {
        Update::NewMessage(message) if message.text() == "/start" => {
            message
                .respond(InputMessage::text("Here's a fibonacci").reply_markup(&fib_markup(0, 1)))
                .await?;
        }
        Update::CallbackQuery(mut query) => {
            let data = std::str::from_utf8(query.data()).unwrap();
            println!("Got callback query for {}", data);

            // First check special-case.
            if data == "done" {
                query.answer().edit("Glad you liked it 👍".into()).await?;
                return Ok(());
            }

            // Otherwise get the stored number(s).
            let mut parts = data.split(',');
            let a = parts.next().unwrap().parse::<u128>().unwrap();
            if let Some(b) = parts.next() {
                let os = (0..b.len()).map(|_| 'o').collect::<String>();
                let b = b.parse::<u128>().unwrap();
                query
                    .answer()
                    .edit(
                        InputMessage::text(&format!("S{} much fibonacci 🔢", os))
                            .reply_markup(&fib_markup(a, b)),
                    )
                    .await?;
            } else if a % 2 == 0 {
                query.answer().text("Even that's a number!").send().await?;
            } else {
                query.answer().alert("That's odd…").send().await?;
            }
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
    let api_hash = env!("TG_HASH").to_string();
    let token = env::args().skip(1).next().expect("token missing");

    println!("Connecting to Telegram...");
    let mut client = Client::connect(Config {
        session: Session::load_file_or_create(SESSION_FILE)?,
        api_id,
        api_hash: api_hash.clone(),
        params: Default::default(),
    })
    .await?;
    println!("Connected!");

    if !client.is_authorized().await? {
        println!("Signing in...");
        client.bot_sign_in(&token, api_id, &api_hash).await?;
        client.session().save_to_file(SESSION_FILE)?;
        println!("Signed in!");
    }

    println!("Waiting for messages...");

    // This code uses `select!` on Ctrl+C to gracefully stop the client and have a chance to
    // save the session. You could have fancier logic to save the session if you wanted to
    // (or even save it on every update). Or you could also ignore Ctrl+C and just use
    // `while let Some(updates) =  client.next_updates().await?`.
    while let Some(update) = client.next_update().await? {
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
