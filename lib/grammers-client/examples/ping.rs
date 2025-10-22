//! This example sends a ping to Telegram through raw API, and that's it.
//!
//! ```sh
//! cargo run --example ping
//! ```

use std::sync::Arc;

use grammers_client::session::Session;
use grammers_client::{Client, Config};
use grammers_mtsender::SenderPool;
use grammers_session::storages::TlSession;
use grammers_tl_types as tl;
use tokio::runtime;

type Result = std::result::Result<(), Box<dyn std::error::Error>>;

async fn async_main() -> Result {
    let session = Arc::new(TlSession::load_file_or_create("ping.session")?);

    let (pool, handle, _) = SenderPool::new(
        Arc::clone(&session) as Arc<dyn Session>,
        1,
        Default::default(),
    );
    let pool_task = tokio::spawn(pool.run());

    println!("Connecting to Telegram...");
    let client = Client::connect(Config {
        session: Arc::clone(&session) as Arc<dyn Session>,
        api_id: 1, // not actually logging in, but has to look real
        api_hash: "".to_string(),
        handle: handle.clone(),
        params: Default::default(),
    })
    .await?;
    println!("Connected!");

    println!("Sending ping...");
    dbg!(client.invoke(&tl::functions::Ping { ping_id: 0 }).await?);
    println!("Ping sent successfully!");

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
