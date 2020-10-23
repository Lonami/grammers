//! This example sends a ping to Telegram through raw API, and that's it.
//!
//! ```sh
//! cargo run --example ping
//! ```

use grammers_client::{AuthorizationError, Client, Config};
use grammers_session::Session;
use grammers_tl_types as tl;
use tokio::runtime;

async fn async_main() -> Result<(), AuthorizationError> {
    println!("Connecting to Telegram...");
    let mut client = Client::connect(Config {
        session: Session::load_or_create("ping.session")?,
        api_id: 1, // not actually logging in, but has to look real
        api_hash: "".to_string(),
        params: Default::default(),
    })
    .await?;
    println!("Connected!");

    println!("Sending ping...");
    dbg!(client.invoke(&tl::functions::Ping { ping_id: 0 }).await?);
    println!("Ping sent successfully!");

    Ok(())
}

fn main() -> Result<(), AuthorizationError> {
    runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap()
        .block_on(async_main())
}
