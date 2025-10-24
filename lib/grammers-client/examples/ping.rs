//! This example sends a ping to Telegram through raw API, and that's it.
//!
//! ```sh
//! cargo run --example ping
//! ```

use std::sync::Arc;

use grammers_client::Client;
use grammers_mtsender::SenderPool;
use grammers_session::storages::TlSession;
use grammers_tl_types as tl;
use tokio::runtime;

type Result = std::result::Result<(), Box<dyn std::error::Error>>;

async fn async_main() -> Result {
    let session = Arc::new(TlSession::load_file_or_create("ping.session")?);
    let pool = SenderPool::new(Arc::clone(&session), 1);
    let client = Client::new(&pool);
    let SenderPool { runner, handle, .. } = pool;
    let pool_task = tokio::spawn(runner.run());

    println!("Sending ping...");
    dbg!(client.invoke(&tl::functions::Ping { ping_id: 0 }).await?);
    println!("Ping sent successfully!");

    // Pool's `run()` won't finish until all handles are dropped or quit is called.
    // Note that the pool's `handle` isn't dropped if it were omitted when destructuring.
    //
    // You don't need to explicitly close the connection, but this is a way to do it gracefully.
    drop(handle);
    drop(client);
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
