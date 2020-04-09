//! This example sends a ping to Telegram through raw API, and that's it.
//!
//! ```sh
//! cargo run --example ping
//! ```

use async_std::task;
use grammers_client::{AuthorizationError, Client};
use grammers_tl_types as tl;

async fn async_main() -> Result<(), AuthorizationError> {
    println!("Connecting to Telegram...");
    let mut client = Client::connect().await?;
    println!("Connected!");

    println!("Sending ping...");
    dbg!(client.invoke(&tl::functions::Ping { ping_id: 0 }).await?);
    println!("Ping sent successfully!");

    Ok(())
}

fn main() -> Result<(), AuthorizationError> {
    task::block_on(async_main())
}
