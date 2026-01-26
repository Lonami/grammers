use base64::Engine;
use grammers_client::Client;
use grammers_client::peer::User;
use grammers_mtsender::SenderPool;
use grammers_session::storages::MemorySession;
use std::sync::Arc;

#[tokio::test]
#[ignore]
async fn test_qr_login_functionality() {
    let api_id = std::env::var("API_ID")
        .expect("API_ID must be set")
        .parse::<i32>()
        .expect("API_ID must be a valid integer");
    let api_hash = std::env::var("API_HASH").expect("API_HASH must be set");

    // Create a session
    let session = Arc::new(MemorySession::default());

    // Create a sender pool
    let SenderPool { runner, handle, .. } = SenderPool::new(Arc::clone(&session), api_id);
    let client = Client::new(handle);

    // Spawn the runner
    let _runner_handle = tokio::spawn(runner.run());

    // Wait for QR login with a timeout of 2 minutes
    let user = wait_for_qr_login(
        &client,
        api_id,
        &api_hash,
        std::time::Duration::from_secs(120),
    )
    .await
    .expect("Failed to complete QR login within timeout");

    // Verify successful login
    println!(
        "Successfully logged in as: {:?}",
        user.first_name().unwrap_or("Unknown")
    );

    // Make a test API call to ensure the connection is established on the new DC
    // This helps ensure the client is properly authenticated on the migrated DC
    match client
        .invoke(&grammers_client::tl::functions::updates::GetState {})
        .await
    {
        Ok(_) => println!("GetState call succeeded, connection established"),
        Err(e) => println!("GetState call failed: {}", e),
    }

    // Check authorization status immediately
    let is_auth_immediate = client.is_authorized().await.unwrap_or(false);
    println!("Immediate authorization status: {}", is_auth_immediate);

    // Allow time for session synchronization after migration
    for i in 0..10 {
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
        let is_authorized = client.is_authorized().await.unwrap_or(false);
        println!("Attempt {}: is_authorized = {}", i + 1, is_authorized);

        if is_authorized {
            println!("Authorization confirmed after {} attempts", i + 1);
            break;
        }

        if i == 9 {
            println!("Authorization still not detected after 10 attempts");
        }
    }

    let final_auth_status = client.is_authorized().await.unwrap();
    println!("Final authorization status: {}", final_auth_status);
    assert!(final_auth_status);

    println!("QR login completed successfully!");
}

use std::time::Duration;
use tokio::time::Instant;

async fn wait_for_qr_login(
    client: &Client,
    api_id: i32,
    api_hash: &str,
    max_wait: Duration,
) -> Result<User, Box<dyn std::error::Error>> {
    println!("Starting QR login process...");

    // Get initial QR token
    let mut qr_info = client.start_qr_login(api_id, api_hash).await?;
    println!("QR URL: {}", qr_info.qr_url);
    println!("Expires in: {} seconds", qr_info.expires_in_seconds);

    let start_time = Instant::now();
    let mut current_token = Vec::<u8>::new();
    let mut loop_count = 0;

    loop {
        // Check for timeout
        if start_time.elapsed() >= max_wait {
            return Err(Box::new(grammers_mtsender::InvocationError::Rpc(
                grammers_mtsender::RpcError {
                    code: 408,
                    name: "TIMEOUT".to_string(),
                    value: None,
                    caused_by: None,
                },
            )));
        }

        // Check if we're close to expiration, refresh if needed
        if qr_info.expires_in_seconds <= 5 {
            println!("Refreshing QR token...");
            match client.export_login_token(api_id, api_hash).await {
                Ok(login_token) => {
                    match login_token {
                        grammers_tl_types::enums::auth::LoginToken::Token(token) => {
                            let new_qr_url = format!(
                                "tg://login?token={}",
                                base64::engine::general_purpose::URL_SAFE_NO_PAD
                                    .encode(&token.token)
                            );
                            println!("New QR URL: {}", new_qr_url);

                            let expires_unix = token.expires as u64;
                            let current_time = std::time::SystemTime::now()
                                .duration_since(std::time::UNIX_EPOCH)
                                .unwrap()
                                .as_secs();
                            let expires_in_seconds = token.expires as i64 - current_time as i64;

                            qr_info.qr_url = new_qr_url;
                            qr_info.expires_unix = expires_unix;
                            qr_info.expires_in_seconds = expires_in_seconds;

                            current_token = token.token.clone();
                        }
                        grammers_tl_types::enums::auth::LoginToken::Success(success) => {
                            println!("Login successful via refresh!");
                            match success.authorization {
                                grammers_tl_types::enums::auth::Authorization::Authorization(
                                    auth,
                                ) => {
                                    return client
                                        .finalize_qr_login(auth)
                                        .await
                                        .map_err(|e| Box::new(e) as Box<dyn std::error::Error>);
                                }
                                grammers_tl_types::enums::auth::Authorization::SignUpRequired(
                                    _,
                                ) => {
                                    return Err(Box::new(grammers_mtsender::InvocationError::Rpc(
                                        grammers_mtsender::RpcError {
                                            code: 400,
                                            name: "SIGN_UP_REQUIRED".to_string(),
                                            value: None,
                                            caused_by: None,
                                        },
                                    )));
                                }
                            }
                        }
                        grammers_tl_types::enums::auth::LoginToken::MigrateTo(migrate_to) => {
                            println!("Handling DC migration to DC {}", migrate_to.dc_id);
                            // Handle migration by importing the token on the new DC
                            match client
                                .import_login_token(migrate_to.token, migrate_to.dc_id)
                                .await
                            {
                                Ok(import_result) => {
                                    match import_result {
                                        grammers_tl_types::enums::auth::LoginToken::Success(
                                            success,
                                        ) => {
                                            println!("Login successful after migration!");
                                            match success.authorization {
                                                grammers_tl_types::enums::auth::Authorization::Authorization(auth) => {
                                                    return client.finalize_qr_login(auth).await.map_err(|e| Box::new(e) as Box<dyn std::error::Error>);
                                                }
                                                grammers_tl_types::enums::auth::Authorization::SignUpRequired(_) => {
                                                    return Err(Box::new(grammers_mtsender::InvocationError::Rpc(grammers_mtsender::RpcError {
                                                        code: 400,
                                                        name: "SIGN_UP_REQUIRED".to_string(),
                                                        value: None,
                                                        caused_by: None,
                                                    })));
                                                }
                                            }
                                        }
                                        grammers_tl_types::enums::auth::LoginToken::Token(
                                            token,
                                        ) => {
                                            let new_qr_url = format!(
                                                "tg://login?token={}",
                                                base64::engine::general_purpose::URL_SAFE_NO_PAD
                                                    .encode(&token.token)
                                            );
                                            println!("New QR URL after migration: {}", new_qr_url);

                                            let expires_unix = token.expires as u64;
                                            let current_time = std::time::SystemTime::now()
                                                .duration_since(std::time::UNIX_EPOCH)
                                                .unwrap()
                                                .as_secs();
                                            let expires_in_seconds =
                                                token.expires as i64 - current_time as i64;

                                            qr_info.qr_url = new_qr_url;
                                            qr_info.expires_unix = expires_unix;
                                            qr_info.expires_in_seconds = expires_in_seconds;

                                            current_token = token.token.clone();
                                        }
                                        grammers_tl_types::enums::auth::LoginToken::MigrateTo(
                                            _,
                                        ) => {
                                            println!("Unexpected double migration");
                                            // Continue loop
                                        }
                                    }
                                }
                                Err(grammers_mtsender::InvocationError::Rpc(ref err))
                                    if err.name == "SESSION_PASSWORD_NEEDED" =>
                                {
                                    // Handle 2FA required case after migration
                                    println!(
                                        "2FA password required after migration. Getting password information..."
                                    );
                                    match client.qr_get_password_token().await {
                                        Ok(password_token) => {
                                            if let Some(hint) = password_token.hint() {
                                                println!("Password hint: {}", hint);
                                            }

                                            // Try to get password from environment variable
                                            match std::env::var("TG_2FA_PASSWORD") {
                                                Ok(password) => {
                                                    println!(
                                                        "Using 2FA password from environment variable"
                                                    );
                                                    // Complete login with password
                                                    match client
                                                        .check_password(
                                                            password_token,
                                                            password.as_bytes(),
                                                        )
                                                        .await
                                                    {
                                                        Ok(user) => {
                                                            println!(
                                                                "Successfully logged in with 2FA after migration"
                                                            );
                                                            return Ok(user);
                                                        }
                                                        Err(sign_in_error) => match sign_in_error {
                                                            grammers_client::SignInError::Other(
                                                                invocation_error,
                                                            ) => {
                                                                println!(
                                                                    "Failed to complete login with 2FA after migration: {}",
                                                                    invocation_error
                                                                );
                                                                return Err(Box::new(
                                                                    invocation_error,
                                                                ));
                                                            }
                                                            _ => {
                                                                println!(
                                                                    "Failed to complete login with 2FA after migration: {}",
                                                                    sign_in_error
                                                                );
                                                                return Err(Box::new(
                                                                    sign_in_error,
                                                                ));
                                                            }
                                                        },
                                                    }
                                                }
                                                Err(_) => {
                                                    println!(
                                                        "TG_2FA_PASSWORD environment variable not set. Please set it to complete 2FA login."
                                                    );
                                                    return Err(Box::new(
                                                        grammers_mtsender::InvocationError::Rpc(
                                                            grammers_mtsender::RpcError {
                                                                code: 401,
                                                                name: "SESSION_PASSWORD_NEEDED"
                                                                    .to_string(),
                                                                value: None,
                                                                caused_by: None,
                                                            },
                                                        ),
                                                    ));
                                                }
                                            }
                                        }
                                        Err(get_pw_error) => {
                                            println!(
                                                "Failed to get password information: {}",
                                                get_pw_error
                                            );
                                            return Err(Box::new(get_pw_error));
                                        }
                                    }
                                }
                                Err(e) => {
                                    println!("Failed to import login token after migration: {}", e);
                                    return Err(Box::new(e));
                                }
                            }
                        }
                    }
                }
                Err(grammers_mtsender::InvocationError::Rpc(ref err))
                    if err.name == "SESSION_PASSWORD_NEEDED" =>
                {
                    // Handle 2FA required case
                    println!("2FA password required. Getting password information...");
                    match client.get_password_token().await {
                        Ok(password_token) => {
                            if let Some(hint) = password_token.hint() {
                                println!("Password hint: {}", hint);
                            }

                            // Try to get password from environment variable
                            match std::env::var("TG_2FA_PASSWORD") {
                                Ok(password) => {
                                    println!("Using 2FA password from environment variable");
                                    // Complete login with password
                                    match client
                                        .check_password(password_token, password.as_bytes())
                                        .await
                                    {
                                        Ok(user) => {
                                            println!("Successfully logged in with 2FA");
                                            return Ok(user);
                                        }
                                        Err(sign_in_error) => match sign_in_error {
                                            grammers_client::SignInError::Other(
                                                invocation_error,
                                            ) => {
                                                println!(
                                                    "Failed to complete login with 2FA: {}",
                                                    invocation_error
                                                );
                                                return Err(Box::new(invocation_error));
                                            }
                                            _ => {
                                                println!(
                                                    "Failed to complete login with 2FA: {}",
                                                    sign_in_error
                                                );
                                                return Err(Box::new(sign_in_error));
                                            }
                                        },
                                    }
                                }
                                Err(_) => {
                                    println!(
                                        "TG_2FA_PASSWORD environment variable not set. Please set it to complete 2FA login."
                                    );
                                    return Err(Box::new(grammers_mtsender::InvocationError::Rpc(
                                        grammers_mtsender::RpcError {
                                            code: 401,
                                            name: "SESSION_PASSWORD_NEEDED".to_string(),
                                            value: None,
                                            caused_by: None,
                                        },
                                    )));
                                }
                            }
                        }
                        Err(get_pw_error) => {
                            println!("Failed to get password information: {}", get_pw_error);
                            return Err(Box::new(get_pw_error));
                        }
                    }
                }
                Err(e) => {
                    println!("Failed to refresh token: {}", e);
                    return Err(Box::new(e));
                }
            }
        }

        // Poll for login status
        match client.export_login_token(api_id, api_hash).await {
            Ok(login_token) => {
                match login_token {
                    grammers_tl_types::enums::auth::LoginToken::Token(token) => {
                        // Update QR info if token has changed
                        let token_bytes = &token.token;
                        if token_bytes != &current_token {
                            let new_qr_url = format!(
                                "tg://login?token={}",
                                base64::engine::general_purpose::URL_SAFE_NO_PAD
                                    .encode(token_bytes)
                            );
                            println!("QR token updated: {}", new_qr_url);

                            let expires_unix = token.expires as u64;
                            let current_time = std::time::SystemTime::now()
                                .duration_since(std::time::UNIX_EPOCH)
                                .unwrap()
                                .as_secs();
                            let expires_in_seconds = token.expires as i64 - current_time as i64;

                            qr_info.qr_url = new_qr_url;
                            qr_info.expires_unix = expires_unix;
                            qr_info.expires_in_seconds = expires_in_seconds;

                            current_token = token.token.clone();
                        }
                    }
                    grammers_tl_types::enums::auth::LoginToken::Success(success) => {
                        println!("Login successful!");
                        match success.authorization {
                            grammers_tl_types::enums::auth::Authorization::Authorization(auth) => {
                                return client
                                    .finalize_qr_login(auth)
                                    .await
                                    .map_err(|e| Box::new(e) as Box<dyn std::error::Error>);
                            }
                            grammers_tl_types::enums::auth::Authorization::SignUpRequired(_) => {
                                return Err(Box::new(grammers_mtsender::InvocationError::Rpc(
                                    grammers_mtsender::RpcError {
                                        code: 400,
                                        name: "SIGN_UP_REQUIRED".to_string(),
                                        value: None,
                                        caused_by: None,
                                    },
                                )));
                            }
                        }
                    }
                    grammers_tl_types::enums::auth::LoginToken::MigrateTo(migrate_to) => {
                        println!("Detected DC migration to DC {}", migrate_to.dc_id);
                        // Handle migration by importing the token on the new DC
                        match client
                            .import_login_token(migrate_to.token, migrate_to.dc_id)
                            .await
                        {
                            Ok(import_result) => {
                                match import_result {
                                    grammers_tl_types::enums::auth::LoginToken::Success(
                                        success,
                                    ) => {
                                        println!("Login successful after migration!");
                                        match success.authorization {
                                            grammers_tl_types::enums::auth::Authorization::Authorization(auth) => {
                                                return client.finalize_qr_login(auth).await.map_err(|e| Box::new(e) as Box<dyn std::error::Error>);
                                            },
                                            grammers_tl_types::enums::auth::Authorization::SignUpRequired(_) => {
                                                return Err(Box::new(grammers_mtsender::InvocationError::Rpc(grammers_mtsender::RpcError {
                                                    code: 400,
                                                    name: "SIGN_UP_REQUIRED".to_string(),
                                                    value: None,
                                                    caused_by: None,
                                                })));
                                            }
                                        }
                                    }
                                    grammers_tl_types::enums::auth::LoginToken::Token(token) => {
                                        let new_qr_url = format!(
                                            "tg://login?token={}",
                                            base64::engine::general_purpose::URL_SAFE_NO_PAD
                                                .encode(&token.token)
                                        );
                                        println!("New QR URL after migration: {}", new_qr_url);

                                        let expires_unix = token.expires as u64;
                                        let current_time = std::time::SystemTime::now()
                                            .duration_since(std::time::UNIX_EPOCH)
                                            .unwrap()
                                            .as_secs();
                                        let expires_in_seconds =
                                            token.expires as i64 - current_time as i64;

                                        qr_info.qr_url = new_qr_url;
                                        qr_info.expires_unix = expires_unix;
                                        qr_info.expires_in_seconds = expires_in_seconds;

                                        current_token = token.token.clone();
                                    }
                                    grammers_tl_types::enums::auth::LoginToken::MigrateTo(_) => {
                                        println!("Unexpected double migration");
                                        // Continue loop
                                    }
                                }
                            }
                            Err(grammers_mtsender::InvocationError::Rpc(ref err))
                                if err.name == "SESSION_PASSWORD_NEEDED" =>
                            {
                                // Handle 2FA required case after migration
                                println!(
                                    "2FA password required after migration. Getting password information..."
                                );
                                match client.get_password_token().await {
                                    Ok(password_token) => {
                                        if let Some(hint) = password_token.hint() {
                                            println!("Password hint: {}", hint);
                                        }

                                        // Try to get password from environment variable
                                        match std::env::var("TG_2FA_PASSWORD") {
                                            Ok(password) => {
                                                println!(
                                                    "Using 2FA password from environment variable"
                                                );
                                                // Complete login with password
                                                match client
                                                    .check_password(
                                                        password_token,
                                                        password.as_bytes(),
                                                    )
                                                    .await
                                                {
                                                    Ok(user) => {
                                                        println!(
                                                            "Successfully logged in with 2FA after migration"
                                                        );
                                                        return Ok(user);
                                                    }
                                                    Err(sign_in_error) => match sign_in_error {
                                                        grammers_client::SignInError::Other(
                                                            invocation_error,
                                                        ) => {
                                                            println!(
                                                                "Failed to complete login with 2FA after migration: {}",
                                                                invocation_error
                                                            );
                                                            return Err(Box::new(invocation_error));
                                                        }
                                                        _ => {
                                                            println!(
                                                                "Failed to complete login with 2FA after migration: {}",
                                                                sign_in_error
                                                            );
                                                            return Err(Box::new(sign_in_error));
                                                        }
                                                    },
                                                }
                                            }
                                            Err(_) => {
                                                println!(
                                                    "TG_2FA_PASSWORD environment variable not set. Please set it to complete 2FA login."
                                                );
                                                return Err(Box::new(
                                                    grammers_mtsender::InvocationError::Rpc(
                                                        grammers_mtsender::RpcError {
                                                            code: 401,
                                                            name: "SESSION_PASSWORD_NEEDED"
                                                                .to_string(),
                                                            value: None,
                                                            caused_by: None,
                                                        },
                                                    ),
                                                ));
                                            }
                                        }
                                    }
                                    Err(get_pw_error) => {
                                        println!(
                                            "Failed to get password information: {}",
                                            get_pw_error
                                        );
                                        return Err(Box::new(get_pw_error));
                                    }
                                }
                            }
                            Err(e) => {
                                println!("Failed to import login token after migration: {}", e);
                                return Err(Box::new(e));
                            }
                        }
                    }
                }
            }
            Err(grammers_mtsender::InvocationError::Rpc(ref err))
                if err.name == "SESSION_PASSWORD_NEEDED" =>
            {
                // Handle 2FA required case
                println!("2FA password required. Getting password information...");
                match client.get_password_token().await {
                    Ok(password_token) => {
                        if let Some(hint) = password_token.hint() {
                            println!("Password hint: {}", hint);
                        }

                        // Try to get password from environment variable
                        match std::env::var("TG_2FA_PASSWORD") {
                            Ok(password) => {
                                println!("Using 2FA password from environment variable");
                                // Complete login with password
                                match client
                                    .check_password(password_token, password.as_bytes())
                                    .await
                                {
                                    Ok(user) => {
                                        println!("Successfully logged in with 2FA");
                                        return Ok(user);
                                    }
                                    Err(sign_in_error) => match sign_in_error {
                                        grammers_client::SignInError::Other(invocation_error) => {
                                            println!(
                                                "Failed to complete login with 2FA: {}",
                                                invocation_error
                                            );
                                            return Err(Box::new(invocation_error));
                                        }
                                        _ => {
                                            println!(
                                                "Failed to complete login with 2FA: {}",
                                                sign_in_error
                                            );
                                            return Err(Box::new(sign_in_error));
                                        }
                                    },
                                }
                            }
                            Err(_) => {
                                println!(
                                    "TG_2FA_PASSWORD environment variable not set. Please set it to complete 2FA login."
                                );
                                return Err(Box::new(grammers_mtsender::InvocationError::Rpc(
                                    grammers_mtsender::RpcError {
                                        code: 401,
                                        name: "SESSION_PASSWORD_NEEDED".to_string(),
                                        value: None,
                                        caused_by: None,
                                    },
                                )));
                            }
                        }
                    }
                    Err(get_pw_error) => {
                        println!("Failed to get password information: {}", get_pw_error);
                        return Err(Box::new(get_pw_error));
                    }
                }
            }
            Err(e) => {
                println!("Error polling for login status: {}", e);
                return Err(Box::new(e));
            }
        }

        // Sleep briefly before next check
        tokio::time::sleep(Duration::from_secs(1)).await;
        loop_count += 1;

        // Update expiration countdown
        let elapsed = start_time.elapsed().as_secs();
        let remaining = max_wait.as_secs().saturating_sub(elapsed);
        if loop_count % 5 == 0 {
            // Print every 5 seconds
            println!("Still waiting... {} seconds remaining", remaining);
        }
    }
}

// Add a comment about how to run this test
/*
To run this test:
cargo test test_qr_login_functionality -- --ignored --nocapture
*/
