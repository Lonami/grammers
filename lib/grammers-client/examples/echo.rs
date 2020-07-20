//! Example to echo user text messages. Runnable as:
//!
//! ```sh
//! cargo run --example echo -- API_ID API_HASH BOT_TOKEN
//! ```

use async_std::task;
use grammers_client::types::EntitySet;
use grammers_client::{AuthorizationError, Client, Config, InvocationError};
use grammers_session::Session;
use grammers_tl_types as tl;
use log;
use simple_logger;
use std::env;

async fn handle_updates(
    client: &mut Client,
    updates: tl::enums::Updates,
) -> Result<(), InvocationError> {
    match updates {
        tl::enums::Updates::Updates(tl::types::Updates {
            updates,
            users,
            chats,
            ..
        }) => {
            let entity_set = EntitySet::new_owned(users, chats);

            for update in updates {
                match update {
                    tl::enums::Update::NewMessage(tl::types::UpdateNewMessage {
                        message: tl::enums::Message::Message(message),
                        ..
                    }) => {
                        let peer = if matches!(message.to_id, tl::enums::Peer::User(_)) {
                            // Sent in private, `to_id` is us, build peer from `from_id` instead
                            entity_set.get(&tl::enums::Peer::User(tl::types::PeerUser {
                                user_id: message.from_id.unwrap(),
                            }))
                        } else {
                            entity_set.get(&message.to_id)
                        }
                        .expect("failed to find entity");

                        println!("Responding to {:?}", peer);
                        client
                            .send_message(peer.to_input_peer(), message.message.as_str().into())
                            .await?;
                    }
                    tl::enums::Update::NewChannelMessage(tl::types::UpdateNewChannelMessage {
                        message: tl::enums::Message::Message(message),
                        ..
                    }) => {
                        let peer = entity_set
                            .get(&message.to_id)
                            .expect("failed to find entity");

                        println!("Responding to {:?}", peer);
                        client
                            .send_message(peer.to_input_peer(), message.message.as_str().into())
                            .await?;
                    }
                    _ => {}
                }
            }
        }
        // For simplicity, we're not handling:
        // * UpdateShortMessage
        // * UpdateShortChatMessage
        // * UpdateShort
        _ => {}
    }

    Ok(())
}

async fn async_main() -> Result<(), AuthorizationError> {
    simple_logger::init_with_level(log::Level::Debug).expect("failed to setup logging");

    let mut args = env::args();

    let _path = args.next();
    let api_id = args
        .next()
        .expect("api_id missing")
        .parse()
        .expect("api_id invalid");
    let api_hash = args.next().expect("api_hash missing");
    let token = args.next().expect("token missing");

    println!("Connecting to Telegram...");
    let mut client = Client::connect(Config {
        session: Session::load_or_create("echo.session")?,
        api_id,
        api_hash: api_hash.clone(),
        params: Default::default(),
    })
    .await?;
    println!("Connected!");

    if !client.is_authorized().await? {
        println!("Signing in...");
        client.bot_sign_in(&token, api_id, &api_hash).await?;
        println!("Signed in!");
    }

    println!("Waiting for messages...");
    while let Some(updates) = client.next_updates().await {
        handle_updates(&mut client, updates).await?;
    }

    Ok(())
}

fn main() -> Result<(), AuthorizationError> {
    task::block_on(async_main())
}
