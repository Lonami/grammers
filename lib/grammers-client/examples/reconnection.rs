//! this example demonstrate how to implement custom Reconnection Polies

use grammers_client::session::Session;
use grammers_client::{Client, Config, InitParams, ReconnectionPolicy};
use std::ops::ControlFlow;
use std::time::Duration;
use tokio::runtime;

type Result = std::result::Result<(), Box<dyn std::error::Error>>;

/// note that this can contain any value you need, in this case, its empty
struct MyPolicy;

impl ReconnectionPolicy for MyPolicy {
    ///this is the only function you need to implement,
    /// it gives you the attempted reconnections, and `self` in case you have any data in your struct.
    /// you should return a [`ControlFlow`] which can be either `Break` or `Continue`, break will **NOT** attempt a reconnection,
    /// `Continue` **WILL** try to reconnect after the given **Duration**.
    ///
    /// in this example we are simply sleeping exponentially based on the attempted count,
    /// however this is not a really good practice for production since we are just doing 2 raised to the power of attempts and that will result to massive
    /// numbers very soon, just an example!
    fn should_retry(&self, attempts: usize) -> ControlFlow<(), Duration> {
        let duration = u64::pow(2, attempts as _);
        ControlFlow::Continue(Duration::from_millis(duration))
    }
}

async fn async_main() -> Result {
    println!("Connecting to Telegram...");
    let client = Client::connect(Config {
        session: Session::load_file_or_create("ping.session")?,
        api_id: 1, // not actually logging in, but has to look real
        api_hash: "".to_string(),
        params: InitParams {
            reconnection_policy: &MyPolicy,
            ..Default::default()
        },
    })
    .await?;

    /// happy listening to updates forever!!
    use grammers_client::Update;

    while let Some(update) = client.next_update().await? {
        match update {
            Update::NewMessage(message) if !message.outgoing() => {
                message.respond(message.text()).await?;
            }
            _ => {}
        }
    }
    Ok(())
}

fn main() -> Result {
    runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap()
        .block_on(async_main())
}
