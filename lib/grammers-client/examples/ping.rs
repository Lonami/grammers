//! This example sends a ping to Telegram through raw API, and that's it.
//!
//! ```sh
//! cargo run --example ping
//! ```

use grammers_client::Client;
use grammers_tl_types as tl;
use std::io::Result;

fn main() -> Result<()> {
    println!("Connecting to Telegram...");
    let mut client = Client::new()?;
    println!("Connected!");

    println!("Sending ping...");
    client.invoke(&tl::functions::Ping { ping_id: 0 })?;
    println!("Ping sent successfully!");

    Ok(())
}
