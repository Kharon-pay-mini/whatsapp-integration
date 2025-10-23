use actix_web::{HttpResponse, Result, web};
use base64::{Engine as _, engine::general_purpose::STANDARD as Engine};
use chrono::{DateTime, Utc};
use serde_json::Value;
use std::{collections::HashMap, sync::Mutex, time::Duration};
use tokio::time::sleep;

use crate::model::{
    BankDetails, BankListResponse, BankVerificationResponse, CreateControllerAPIResponse,
    InitDisbursementResponse, ReceivePaymentRequest, UserSessions, UserState,
    WebhookStatusResponse,
};

pub type SessionMap = HashMap<String, UserSessions>;

pub async fn health_check() -> actix_web::Result<HttpResponse> {
    Ok(HttpResponse::Ok().json(serde_json::json!({
        "status": "healthy",
        "service": "Kharon Pay WhatsApp Bot"
    })))
}

pub async fn handle_twilio_webhook(
    body: web::Bytes,
    sessions: web::Data<std::sync::Mutex<SessionMap>>,
) -> Result<HttpResponse> {
    let form_data: HashMap<String, String> = match serde_urlencoded::from_bytes(&body) {
        Ok(data) => data,
        Err(_) => return Ok(HttpResponse::BadRequest().body("Invalid form data")),
    };

    // Filter out status webhooks (delivered, read, sent, etc.)
    if let Some(status) = form_data
        .get("SmsStatus")
        .or(form_data.get("MessageStatus"))
    {
        if matches!(
            status.as_str(),
            "delivered" | "read" | "sent" | "failed" | "undelivered"
        ) {
            return Ok(HttpResponse::Ok()
                .content_type("application/xml")
                .body("<?xml version=\"1.0\" encoding=\"UTF-8\"?><Response></Response>"));
        }
    }

    let from = match form_data.get("From") {
        Some(f) => f.clone(),
        None => return Ok(HttpResponse::BadRequest().body("Missing 'From' field")),
    };

    let body_text = form_data.get("Body").cloned().unwrap_or_default();

    // Skip empty messages
    if body_text.trim().is_empty() {
        return Ok(HttpResponse::Ok()
            .content_type("application/xml")
            .body("<?xml version=\"1.0\" encoding=\"UTF-8\"?><Response></Response>"));
    }

    let user_phone = from.replace("whatsapp:", "");

    // Prevent loops - ignore messages from our own bot number
    let bot_number = std::env::var("T_WHATSAPP_NUMBER").unwrap_or_default();
    if user_phone.replace("+", "") == bot_number.replace("whatsapp:", "").replace("+", "") {
        return Ok(HttpResponse::Ok()
            .content_type("application/xml")
            .body("<?xml version=\"1.0\" encoding=\"UTF-8\"?><Response></Response>"));
    }

    handle_message(&user_phone, &body_text, sessions).await;

    Ok(HttpResponse::Ok()
        .content_type("application/xml")
        .body("<?xml version=\"1.0\" encoding=\"UTF-8\"?><Response></Response>"))
}

async fn handle_message(
    user_phone: &str,
    message_text: &str,
    sessions: web::Data<Mutex<SessionMap>>,
) {
    let mut session_guard = sessions.lock().unwrap();
    let session = session_guard
        .entry(user_phone.to_string())
        .or_insert(UserSessions {
            phone: user_phone.to_string(),
            state: UserState::Initial,
            account_id: None,
            pending_amount: None,
            pending_currency: None,
            controller_address: None,
            pending_bank_details: None,
            pending_bank_verification: None,
        });

    let messages = match &session.state {
        UserState::Initial => handle_commands(message_text, session).await,

        UserState::AccountCreation => handle_account_creation(message_text, session).await,

        UserState::OfframpConfirmation => {
            vec![handle_offramp_confirmation(message_text, session).await]
        }

        UserState::SavedBankConfirmation => {
            vec![handle_saved_bank_confirmation(message_text, session).await]
        }

        UserState::BankDetailsEntry => {
            vec![handle_new_bank_details_entry(message_text, session).await]
        }

        UserState::BankDetailsConfirmation => {
            vec![handle_new_bank_confirmation(message_text, session).await]
        }
    };

    // Release the lock before sending messages
    drop(session_guard);

    for (i, message) in messages.iter().enumerate() {
        // Optional: Add delay between multiple messages
        if i > 0 {
            tokio::time::sleep(tokio::time::Duration::from_millis(1000)).await;
        }
        send_twilio_message(user_phone, message).await;
    }
}

async fn handle_commands(message: &str, session: &mut UserSessions) -> Vec<String> {
    let parts: Vec<&str> = message.split_whitespace().collect();
    if parts.is_empty() {
        return vec!["❓ Unknown command. Type `help` for available commands.".to_string()];
    }
    match parts[0].to_lowercase().as_str() {
        msg if msg.contains("hi") || msg.contains("hello") || msg.contains("start") => {
            vec!["🟢 Welcome to *Kharon Pay*! 💰\n\nSend crypto to your bank in seconds.\n\n📱 *Commands:*\n• `create` - Create new account\n• `fund` - Deposit crypto to your wallet address\n• `withdraw` - Send crypto to your bank account\n• `balance` - Check crypto balance in your wallet\n• `help` - Show all commands\n\nWhat would you like to do?".to_string()]
        }
        "create" => {
            session.state = UserState::AccountCreation;

            let message_clone = message.to_string();
            let mut session_clone = session.clone();

            tokio::spawn(async move {
                let _ = handle_account_creation(&message_clone, &mut session_clone).await;
            });

            vec![]
        }
        "address" => handle_get_address(session).await,
        "balance" => {
            vec![handle_get_balance(session).await]
        }
        "withdraw" => {
            if parts.len() >= 3 {
                if let Ok(amount) = parts[1].parse::<f64>() {
                    let crypto = parts[2].to_uppercase();

                    session.pending_amount = Some(amount);
                    session.pending_currency = Some(crypto.clone());

                    vec![handle_withdraw_initiation(amount, &crypto, session).await]
                } else {
                    vec![
                        "❌ Invalid amount. Use format: `send [amount] [crypto] to [bank name]`"
                            .to_string(),
                    ]
                }
            } else {
                vec!["💸 *Withdraw Format:*\n`send [amount] [crypto] to [bank name]`\n\n*Example:* `send 1 USDT to Opay`".to_string()]
            }
        }
        "help" => {
            vec!["🔰 *Kharon Pay Help*\n\n*Commands:*\n• `create` - Create new account\n• `address` - Get your wallet address\n• `balance` - Check crypto balance\n• `send [amount] [crypto] to [bank name]` - Send to bank\n\n*Examples:*\n• `send 100 USDT to Opay`\n• `balance`\n• `address`".to_string()]
        }
        _ => vec![
            "❓ I didn't understand that. Type `help` for available commands or `hi` to start."
                .to_string(),
        ],
    }
}

async fn handle_account_creation(message: &str, session: &mut UserSessions) -> Vec<String> {
    let create_endpoint = std::env::var("SERVER_CREATE_ENDPOINT").unwrap_or_default();
    let controller_create_endpoint =
        std::env::var("SERVER_CREATE_CONTROLLER_ENDPOINT").unwrap_or_default();
    let api_key = std::env::var("HMAC_KEY").unwrap_or_default();

    let client = match reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(120))
        .build()
    {
        Ok(client) => client,
        Err(e) => {
            eprintln!("Failed to build HTTP client: {}", e);
            return vec!["❌ Account creation failed. Please try again.".to_string()];
        }
    };

    let formatted_phone = session.phone.trim_start_matches("+");

    let account_create_message =
        "🔄 *Creating Your Account!*\n\nPlease wait while we set up your wallet...";
    send_twilio_message(formatted_phone, account_create_message).await;

    let response = client
        .post(create_endpoint)
        .header("x-api-key", &api_key)
        .header("x-service", "whatsapp-bot")
        .json(&serde_json::json!({
            "username": message,
            "service_type": "whatsapp",
            "phone": &formatted_phone,
        }))
        .send()
        .await;

    match response {
        Ok(res) => {
            if res.status().is_success() {
                println!("Account creation request successful!");

                let controller_response = client
                    .post(&controller_create_endpoint)
                    .header("x-api-key", &api_key)
                    .header("x-service", "whatsapp-bot")
                    .timeout(std::time::Duration::from_secs(120))
                    .json(&serde_json::json!({
                        "username": message,
                        "service_type": "whatsapp",
                        "phone": formatted_phone,
                        "user_permission": ["user"],
                    }))
                    .send()
                    .await;

                match controller_response {
                    Ok(controller_res) if controller_res.status().is_success() => {
                        println!("Controller creation successful!");

                        match controller_res.json::<CreateControllerAPIResponse>().await {
                            Ok(response) => {
                                let controller_address = response.data.controller_address;
                                session.controller_address = Some(controller_address.clone());
                                session.state = UserState::Initial;
                                println!("Controller Address: {}", controller_address);

                                send_twilio_message(formatted_phone, &controller_address).await;

                                sleep(Duration::from_millis(800)).await;

                                let success_msg = format!(
                                    "🎉 *Account created successfully!*\n\n\
                                    📱 *To withdraw crypto:*\n\
                                    • `copy address` - Copy your wallet address above\n\
                                    • `fund account` - Send crypto to your wallet address.\n\
                                    • `withdraw` - Send crypto to your bank account."
                                );
                                send_twilio_message(formatted_phone, &success_msg).await;

                                vec![]
                            }
                            Err(parse_err) => {
                                eprintln!("Failed to parse controller response: {}", parse_err);
                                vec!["❌ Account creation failed during controller setup. Please try again.".to_string()]
                            }
                        }
                    }
                    Ok(controller_res) => {
                        eprintln!(
                            "Controller creation failed with status: {}",
                            controller_res.status()
                        );
                        vec!["❌ Account creation failed. Please contact support.".to_string()]
                    }
                    Err(err) => {
                        eprintln!("Controller creation error: {}", err);
                        vec!["❌ Account creation failed. Please try again.".to_string()]
                    }
                }
            } else {
                eprintln!(
                    "Account creation request failed with status: {}",
                    res.status()
                );
                vec!["❌ Account creation failed. Please try again.".to_string()]
            }
        }
        Err(err) => {
            eprintln!("Account creation request error: {}", err);
            vec!["❌ Account creation failed. Please try again.".to_string()]
        }
    }
}

async fn handle_get_address(session: &UserSessions) -> Vec<String> {
    let address_endpoint = std::env::var("SERVER_GET_ADDRESS_ENDPOINT").unwrap_or_default();
    let api_key = std::env::var("HMAC_KEY").unwrap_or_default();

    let client = match reqwest::Client::builder()
        .timeout(Duration::from_secs(130))
        .build()
    {
        Ok(client) => client,
        Err(e) => {
            eprintln!("Failed to build HTTP client: {}", e);
            return vec!["❌ Failed to connect to server. Please try again.".to_string()];
        }
    };

    let formatted_phone = session.phone.trim_start_matches("+");

    let response = client
        .get(&address_endpoint)
        .header("x-api-key", &api_key)
        .header("x-service", "whatsapp-bot")
        .query(&[("phone", &formatted_phone)])
        .send()
        .await;

    match response {
        Ok(res) if res.status().is_success() => {
            match res.json::<Value>().await {
                Ok(data) => {
                    if let Some(address) = data
                        .get("data")
                        .and_then(|d| d.get("controller_address"))
                        .and_then(|a| a.as_str())
                    {
                        vec![
                    address.to_string(),
                    "💳 *Your Wallet Address:*\n\n⚠️ *Only send USDT/USDC (Starknet) to this address*".to_string()

                   ]
                    } else {
                        vec!["❌ No wallet address found. Please create an account first with `create`."
                        .to_string()]
                    }
                }
                Err(_) => vec!["❌ Failed to retrieve address. Please try again.".to_string()],
            }
        }
        Ok(res) if res.status().as_u16() == 404 => {
            vec!["❌ No account found. Please create an account first with `create`.".to_string()]
        }
        Ok(_) => vec!["❌ Failed to retrieve address. Please try again.".to_string()],
        Err(_) => vec!["❌ Failed to connect to server. Please try again.".to_string()],
    }
}

async fn handle_get_balance(session: &UserSessions) -> String {
    let balance_endpoint = std::env::var("SERVER_BALANCE_ENDPOINT").unwrap_or_default();
    let api_key = std::env::var("HMAC_KEY").unwrap_or_default();

    let client = match reqwest::Client::builder()
        .timeout(Duration::from_secs(130))
        .build()
    {
        Ok(client) => client,
        Err(e) => {
            eprintln!("Failed to build HTTP client: {}", e);
            return "❌ Failed to connect to server. Please try again.".to_string();
        }
    };

    let formatted_phone = session.phone.trim_start_matches("+");
    let query_token = std::env::var("TEST_TOKEN").unwrap();
    let user_address = std::env::var("TEST_ADDRESS").unwrap();

    let response = client
        .get(&balance_endpoint)
        .header("x-api-key", &api_key)
        .header("x-service", "whatsapp-bot")
        .query(&[
            ("phone", formatted_phone),
            ("token", &query_token),
            ("user_address", &user_address),
        ])
        .send()
        .await;

    match response {
        Ok(res) if res.status().is_success() => match res.json::<Value>().await {
            Ok(data) => {
                if let Some(balance_str) = data
                    .get("data")
                    .and_then(|b| b.get("balance"))
                    .and_then(|b| b.as_str())
                {
                    if let Ok(balance) = balance_str.parse::<f64>() {
                        let token = data
                            .get("data")
                            .and_then(|d| d.get("token"))
                            .and_then(|t| t.as_str())
                            .unwrap_or("unknown");

                        let (emoji, symbol) = get_token_display(token);

                        format!(
                            "💰 *Your Balance*\n\n{} {}: {:.2}\n\n💵 Total: ${:.2}",
                            emoji, symbol, balance, balance
                        )
                    } else {
                        "❌ Invalid balance format".to_string()
                    }
                } else {
                    "💰 *Your Balance*\n\n🪙 USDT: 0.00\n🪙 USDC: 0.00\n\n💵 Total: $0.00"
                        .to_string()
                }
            }
            Err(_) => "❌ Failed to retrieve balance. Please try again.".to_string(),
        },
        Ok(res) if res.status().as_u16() == 404 => {
            "❌ No account found. Please create an account first with `create`.".to_string()
        }
        Ok(_) => "❌ Failed to retrieve balance. Please try again.".to_string(),
        Err(_) => "❌ Failed to connect to server. Please try again.".to_string(),
    }
}

fn get_token_display(token: &str) -> (&'static str, &'static str) {
    match token.to_lowercase().as_str() {
        "0x07d54bad6d6fcff799133a8c0b1fb8120876bb080d75cd601a5c68164d6f6d75" => ("💵", "USDT"),
        _ => ("🪙", "TOKEN"),
    }
}

async fn handle_withdraw_initiation(
    amount: f64,
    crypto: &str,
    session: &mut UserSessions,
) -> String {
    let rate_endpoint = std::env::var("SERVER_RATE_ENDPOINT").unwrap_or_default();
    let api_key = std::env::var("HMAC_KEY").unwrap_or_default();

    let client = match reqwest::Client::builder()
        .timeout(Duration::from_secs(130))
        .build()
    {
        Ok(client) => client,
        Err(_) => {
            return "❌ Failed to connect to server. Please try again.".to_string();
        }
    };

    let response = client
        .get(rate_endpoint)
        .header("x-api-key", &api_key)
        .header("x-service", "whatsapp-bot")
        .send()
        .await;

    match response {
        Ok(res) if res.status().is_success() => match res.json::<Value>().await {
            Ok(data) => {
                if let Some(rate) = data
                    .get("data")
                    .and_then(|d| d.get("usd_ngn_rate"))
                    .and_then(|r| r.as_f64())
                {
                    let naira_amount = amount * rate;

                    session.state = UserState::OfframpConfirmation;

                    format!(
                        "💸 *Withdraw Request*\n\n\
                            Amount: {:.2} {}\n\
                            Rate: ₦{:.2} per {}\n\
                            You'll receive: ₦{:.2}\n\n\
                            Type `confirm` to proceed or `cancel` to abort.",
                        amount, crypto, rate, crypto, naira_amount
                    )
                } else {
                    "❌ Failed to get exchange rate. Please try again.".to_string()
                }
            }
            Err(_) => "❌ Failed to get exchange rate. Please try again.".to_string(),
        },
        Ok(_) => "❌ Failed to get exchange rate. Please try again.".to_string(),
        Err(_) => "❌ Failed to connect to server. Please try again.".to_string(),
    }
}

/*
fn handle_deposit_flow(message: &str, session: &mut UserSessions) -> String {
    let crypto = message.to_uppercase();
    session.state = UserState::DepositFlow;

    // fund account: should return user's controller address with the right token to deposit

    match crypto.as_str() {
        "USDT" => "💵 *USDT Deposit Address (STARKNET)*\n\n\
            `0x04718f5a0fc34cc1af16a1cdee98ffb20c31f5cd61d6ab07201858f4287c938d`\n\n\
            ⚠️ *Important:*\n\
            • Only send USDT (STARKNET) to this address\n\
            • Minimum deposit: 1 USDT\n\
            💬 Reply `balance` after sending to check status."
            .to_string(),
        "USDC" => "💵 *USDC Deposit Address (STARKNET)*\n\n\
            `0x04718f5a0fc34cc1af16a1cdee98ffb20c31f5cd61d6ab07201858f4287c938d`\n\n\
            ⚠️ *Important:*\n\
            • Only send USDC (STARKNET) to this address\n\
            • Minimum deposit: 1 USDC\n\
            💬 Reply `balance` after sending to check status."
            .to_string(),
        _ => "❌ Unsupported crypto. We support `USDT` and `USDC` for now.".to_string(),
    }
}       */

async fn handle_offramp_confirmation(message: &str, session: &mut UserSessions) -> String {
    match message.to_lowercase().as_str() {
        "confirm" => {
            match get_user_bank_details(session).await {
                Ok(banks) => {
                    if let Some(bank_details) = banks.into_iter().next() {
                        session.pending_bank_details = Some(bank_details.clone());
                        session.state = UserState::SavedBankConfirmation;

                        format!(
                            "🏦 *Your Saved Bank Details:*\n\n\
                            Bank: {}\n\
                            Account Name: {}\n\
                            Account Number: {}\n\n\
                            Proceed with this account?\n\
                            Type `yes` to confirm or `no` to cancel.",
                            bank_details.bank_name,
                            bank_details.account_name,
                            bank_details.account_number
                        )
                    } else {
                        session.state = UserState::BankDetailsEntry;
                        "🏦 *Bank Details Required*\n\nPlease provide your bank details in this format:\n\n`Bank Name, Account Number`\n\n*Example:* `Opay, 0123456789`".to_string()
                    }
                }
                Err(e) => {
                    // Error during the API call (e.g., network error)
                    format!("❌ Failed to check bank details: {}", e)
                }
            }
        }
        "cancel" => {
            clear_session(session);
            "❌ *Withdrawal Cancelled*\n\nYour withdrawal request has been cancelled. Type `send [amount] [crypto] to [bank name]` to start again.".to_string()
        }
        _ => "❓ Please type `confirm` to proceed or `cancel` to abort.".to_string(),
    }
}

async fn handle_new_bank_details_entry(message: &str, session: &mut UserSessions) -> String {
    let parts: Vec<&str> = message.split(',').map(|s| s.trim()).collect();

    if parts.len() != 2 {
        return "❌ Invalid format. Please provide bank details in this format:\n\n`Bank Name, Account Number`\n\n*Example:* `Opay, 0123456789`".to_string();
    }

    let bank_name = parts[0];
    let account_number = parts[1];

    if account_number.len() < 10 || !account_number.chars().all(|c| c.is_numeric()) {
        return "❌ Invalid account number. Must be at least 10 digits.".to_string();
    }

    match verify_bank_details(bank_name, account_number, session).await {
        Ok(verification) => {
            session.pending_bank_verification = Some(verification.clone());
            session.state = UserState::BankDetailsConfirmation;

            format!(
                "✅ *Account Verified!*\n\n\
                🏦 Bank: {}\n\
                👤 Account Name: {}\n\
                🔢 Account Number: {}\n\n\
                Is this correct?\n\
                Type `yes` to confirm or `no` to re-enter.",
                verification.bank_name, verification.account_name, verification.account_number
            )
        }
        Err(err) => format!(
            "❌ *Verification Failed*\n\n{}\n\n\
            Please check your bank details and try again.",
            err
        ),
    }
}

async fn handle_saved_bank_confirmation(message: &str, session: &mut UserSessions) -> String {
    match message.to_lowercase().as_str() {
        "yes" => {
            let bank_details = match session.pending_bank_details.clone() {
                Some(details) => {
                    // println!("session details {:#?}", details.clone());
                    details
                }
                None => {
                    return "❌ Bank details not found. Please start again.".to_string();
                }
            };

            execute_offramp(session, &bank_details).await
        }
        "no" => {
            clear_session(session);
            "❌ *Withdrawal Cancelled*\n\n\
            Type `withdraw [amount] [crypto]` to start again."
                .to_string()
        }
        _ => "❓ Please type `yes` to confirm or `no` to cancel.".to_string(),
    }
}

async fn handle_new_bank_confirmation(message: &str, session: &mut UserSessions) -> String {
    match message.to_lowercase().as_str() {
        "yes" => {
            let verification = match session.pending_bank_verification.clone() {
                Some(v) => v,
                None => {
                    return "❌ Verification data not found. Please re-enter your bank details."
                        .to_string();
                }
            };

            match save_bank_details_to_db(session, &verification).await {
                Ok(_) => match get_user_bank_details(session).await {
                    Ok(banks) => {
                        if let Some(bank_details) = banks.into_iter().next() {
                            session.pending_bank_verification = None;

                            execute_offramp(session, &bank_details).await
                        } else {
                            "❌ Failed to retrieve saved bank details (list was empty). Please contact support.".to_string()
                        }
                    }
                    Err(err) => {
                        format!("❌ Error retrieving bank details: {}", err)
                    }
                },
                Err(err) => format!("❌ Failed to save bank details: {}", err),
            }
        }
        "no" => {
            session.state = UserState::BankDetailsEntry;
            session.pending_bank_verification = None;
            "🔄 *Please re-enter Bank Details*\n\nPlease provide your bank details in this format:\n\n`Bank Name, Account Number`\n\n*Example:* `Opay, 0123456789`".to_string()
        }
        _ => "❓ Please type `yes` to confirm or `no` to re-enter.".to_string(),
    }
}

async fn verify_bank_details(
    bank_name: &str,
    account_number: &str,
    session: &mut UserSessions,
) -> Result<BankVerificationResponse, String> {
    let bank_verification_endpoint =
        std::env::var("SERVER_BANK_ACCOUNT_VERIFY_ENDPOINT").unwrap_or_default();
    let api_key = std::env::var("HMAC_KEY").unwrap_or_default();

    let client = match reqwest::Client::builder()
        .timeout(Duration::from_secs(130))
        .build()
    {
        Ok(client) => client,
        Err(e) => {
            eprintln!("Failed to build HTTP client: {}", e);
            return Err("Failed to connect to server. Please try again.".to_string());
        }
    };

    let formatted_phone = session.phone.trim_start_matches("+");
    let response = client
        .post(&bank_verification_endpoint)
        .header("x-api-key", &api_key)
        .header("x-service", "whatsapp-bot")
        .query(&[
            ("phone", formatted_phone),
            ("bank_name", bank_name),
            ("account_number", account_number),
        ])
        .send()
        .await;

    match response {
        Ok(res) if res.status().is_success() => match res.json::<Value>().await {
            Ok(data) => {
                if let (
                    Some(account_name),
                    Some(account_number),
                    Some(bank_name),
                    Some(bank_code),
                ) = (
                    data.get("data")
                        .and_then(|d| d.get("account_name"))
                        .and_then(|n| n.as_str()),
                    data.get("data")
                        .and_then(|d| d.get("account_number"))
                        .and_then(|n| n.as_str()),
                    data.get("data")
                        .and_then(|d| d.get("bank_name"))
                        .and_then(|n| n.as_str()),
                    data.get("data")
                        .and_then(|d| d.get("bank_code"))
                        .and_then(|n| n.as_str()),
                ) {
                    Ok(BankVerificationResponse {
                        account_name: account_name.to_string(),
                        account_number: account_number.to_string(),
                        bank_name: bank_name.to_string(),
                        bank_code: bank_code.to_string(),
                    })
                } else {
                    println!("Invalid bank verification response format: {:?}", data);
                    Err("Invalid bank details received. Please try again.".to_string())
                }
            }
            Err(e) => {
                eprintln!("Failed to parse bank verification response: {}", e);
                Err("Failed to verify bank details. Please try again.".to_string())
            }
        },
        Ok(res) if res.status().as_u16() == 404 => {
            Err("Account not found. Please check your details and try again.".to_string())
        }
        Ok(res) => {
            eprintln!("Bank verification failed with status: {}", res.status());
            Err(format!("Verification failed: {}", res.status()))
        }
        Err(e) => {
            eprintln!("Verification request error: {}", e);
            Err("Failed to connect to server. Please try again.".to_string())
        }
    }
}

pub async fn get_user_bank_details(session: &UserSessions) -> Result<Vec<BankDetails>, String> {
    let bank_details_endpoint =
        std::env::var("SERVER_BANK_ACCOUNT_GETTER_ENDPOINT").unwrap_or_default();
    let api_key = std::env::var("HMAC_KEY").unwrap_or_default();

    let client = match reqwest::Client::builder()
        .timeout(Duration::from_secs(130))
        .build()
    {
        Ok(client) => client,
        Err(e) => {
            eprintln!("Failed to build HTTP client: {}", e);
            return Err("Failed to connect to server. Please try again.".to_string());
        }
    };

    let formatted_phone = session.phone.trim_start_matches("+");
    let response = client
        .get(&bank_details_endpoint)
        .header("x-api-key", &api_key)
        .header("x-service", "whatsapp-bot")
        .query(&[("phone", &formatted_phone)])
        .send()
        .await;

    match response {
        Ok(res) if res.status().is_success() => match res.json::<BankListResponse>().await {
            Ok(parsed_response) => Ok(parsed_response.data.banks),
            Err(e) => {
                eprintln!("Failed to parse bank details response: {}", e);
                Err("Failed to parse bank details. Please try again.".to_string())
            }
        },
        Ok(res) if res.status().as_u16() == 404 => Ok(vec![]),
        Ok(res) => {
            eprintln!(
                "Failed to retrieve bank details with status: {}",
                res.status()
            );
            Err("Failed to retrieve bank details. Please try again.".to_string())
        }
        Err(e) => {
            eprintln!("Bank details request error: {}", e);
            Err("Failed to connect to server. Please try again.".to_string())
        }
    }
}

async fn save_bank_details_to_db(
    session: &UserSessions,
    verification: &BankVerificationResponse,
) -> Result<(), String> {
    let bank_details_save_endpoint =
        std::env::var("SERVER_BANK_DETAILS_CONFIRM_ENDPOINT").unwrap_or_default();
    let api_key = std::env::var("HMAC_KEY").unwrap_or_default();

    let client = match reqwest::Client::builder()
        .timeout(Duration::from_secs(130))
        .build()
    {
        Ok(client) => client,
        Err(e) => {
            eprintln!("Failed to build HTTP client: {}", e);
            return Err("Failed to connect to server. Please try again.".to_string());
        }
    };

    let formatted_phone = session.phone.trim_start_matches("+");
    let response = client
        .post(&bank_details_save_endpoint)
        .header("x-api-key", &api_key)
        .header("x-service", "whatsapp-bot")
        .json(&serde_json::json!({
            "phone": formatted_phone,
            "account_name": verification.account_name,
            "account_number": verification.account_number,
            "bank_code": verification.bank_code,
            "bank_name": verification.bank_name,
        }))
        .send()
        .await;

    match response {
        Ok(res) if res.status().is_success() => {
            println!("Bank details saved successfully!");
            Ok(())
        }
        Ok(res) => {
            eprintln!("Failed to save bank details with status: {}", res.status());
            Err("Failed to save bank details. Please try again.".to_string())
        }
        Err(_) => Err("Failed to connect to server. Please try again.".to_string()),
    }
}

async fn execute_offramp(session: &mut UserSessions, bank_details: &BankDetails) -> String {
    let amount = session.pending_amount.unwrap();
    let crypto = session.pending_currency.clone().unwrap_or_default();

    match initiate_offramp_process(session, bank_details).await {
        Ok(_) => {
            // Reset session state
            clear_session(session);

            format!(
                "✅ *Withdrawal Request Submitted!*\n\n\
                📊 *Details:*\n\
                • Amount: {:.2} {}\n\
                • Bank: {}\n\
                • Account: {} ({})\n\n\
                ⏳ Processing time: 30-60 seconds\n\
                📱 You'll receive a confirmation message when completed, standby",
                amount,
                crypto,
                bank_details.bank_name,
                bank_details.account_number,
                bank_details.account_name
            )
        }
        Err(err) => {
            format!(
                "❌ *Withdrawal Failed*\n\n{}\n\nPlease try again or contact support.",
                err
            )
        }
    }
}

pub async fn initiate_offramp_process(
    session: &UserSessions,
    bank_details: &BankDetails,
) -> Result<String, String> {
    let amount = session
        .pending_amount
        .ok_or_else(|| "Missing pending amount in session".to_string())?;
    let crypto = session
        .pending_currency
        .clone()
        .ok_or_else(|| "Missing pending currency in session".to_string())?;

    let offramp_endpoint = std::env::var("SERVER_OFFRAMP_INIT_ENDPOINT").unwrap_or_default();
    let api_key = std::env::var("HMAC_KEY").unwrap_or_default();

    let client = match reqwest::Client::builder()
        .timeout(Duration::from_secs(145))
        .build()
    {
        Ok(client) => client,
        Err(e) => {
            eprintln!("Failed to build HTTP client: {}", e);
            return Err("Failed to connect to server".to_string());
        }
    };

    let formatted_phone = session.phone.trim_start_matches("+");

    // 1. Send initiation request
    let response = client
        .post(&offramp_endpoint)
        .header("x-api-key", &api_key)
        .header("x-service", "whatsapp-bot")
        .json(&serde_json::json!({
            "phone": formatted_phone,
            "amount": amount,
            "token_symbol": crypto,
            "bank_account_id": bank_details.bank_details_id,
            "currency": "NGN",
            "order_type": "withdraw",
            "payment_method": "bank_transfer",
        }))
        .send()
        .await;

    match response {
        Ok(res) if res.status().is_success() => {
            let init_response = match res.json::<InitDisbursementResponse>().await {
                Ok(data) => data,
                Err(e) => {
                    eprintln!("Failed to parse offramp init response: {}", e);
                    return Err("Invalid response from server. Try again.".to_string());
                }
            };

            if !init_response.success {
                let error_msg = init_response.error.unwrap_or_else(|| {
                    "Offramp initialization failed due to unknown error.".to_string()
                });
                eprintln!("Offramp init failed: {}", error_msg);
                return Err(error_msg);
            }

            let disbursement_details = init_response.data.ok_or_else(|| {
                "Missing disbursement details in successful response.".to_string()
            })?;

            let token = std::env::var("TEST_TOKEN").unwrap();
            let payment_request = ReceivePaymentRequest {
                token,
                amount: amount.to_string(),
                reference: init_response.reference.clone(),
                phone: formatted_phone.to_string(),
            };

            println!(
                "Offramp request initiated successfully! Reference: {}",
                payment_request.reference
            );

            let initiated_at = Utc::now();

            match trigger_payment(payment_request).await {
                Ok(_) => {
                    let success_msg = format!(
                        "✅ *Withdrawal Successfully Initiated!*\n\n\
                        We have successfully sent *{:.2} {}* to your account:\n\n\
                        🏦 **Bank:** {}\n\
                        👤 **Name:** {}\n\
                        🔢 **Ref:** {}\n\n\
                        The funds should reflect in your account shortly.\n
                        You will get a confirmation message once transaction is completed.",
                        disbursement_details.amount,
                        disbursement_details.currency,
                        disbursement_details.bank_name,
                        disbursement_details.account_name,
                        init_response.reference.clone()
                    );

                    let formatted_phone = session.phone.trim_start_matches("+");
                    start_transaction_polling_task(
                        init_response.reference.clone(),
                        formatted_phone.to_string().clone(),
                        disbursement_details.bank_name.clone(),
                        disbursement_details.account_name.clone(),
                        initiated_at,
                    );

                    Ok(success_msg)
                }
                Err(e) => Err(e),
            }
        }
        Ok(res) => {
            eprintln!("Offramp request failed with status: {}", res.status());
            Err("Failed to initiate withdrawal. Please try again, contact support if error persists.".to_string())
        }
        Err(err) => {
            eprintln!("Offramp request error: {}", err);
            Err("Failed to connect to server. Please try again.".to_string())
        }
    }
}

async fn trigger_payment(payment_request: ReceivePaymentRequest) -> Result<(), String> {
    let payment_endpoint = std::env::var("SERVER_PAYMENT_ENDPOINT").unwrap_or_default();
    let api_key = std::env::var("HMAC_KEY").unwrap_or_default();

    let client = match reqwest::Client::builder()
        .timeout(Duration::from_secs(130))
        .build()
    {
        Ok(client) => client,
        Err(e) => {
            eprintln!("Failed to build HTTP client for payment: {}", e);
            return Err("Failed to connect to payment server.".to_string());
        }
    };

    // The request body is the ReceivePaymentRequest struct itself
    let response = client
        .post(&payment_endpoint)
        .header("x-api-key", &api_key)
        .header("x-service", "whatsapp-bot")
        .json(&payment_request)
        .send()
        .await;

    match response {
        Ok(res) if res.status().is_success() => {
            println!(
                "Payment successfully triggered for reference: {}",
                payment_request.reference
            );
            Ok(())
        }
        Ok(res) => {
            eprintln!(
                "Payment trigger failed with status: {} | Body: {:?}",
                res.status(),
                res.text().await
            );
            Err("Payment confirmation failed. Please contact support.".to_string())
        }
        Err(err) => {
            eprintln!("Payment trigger request error: {}", err);
            Err("Failed to connect to payment server. Please try again.".to_string())
        }
    }
}

async fn poll_and_notify_on_completion(
    reference: String,
    user_phone: String,
    bank_name: String,
    account_name: String,
    initiated_at: DateTime<Utc>,
    max_wait_minutes: u32,
) -> Result<(), String> {
    let status_endpoint = std::env::var("TRANSACTION_STATUS_ENDPOINT")
        .map(|base| format!("{}/transactions/{}/status", base, reference))
        .unwrap();

    let api_key = std::env::var("HMAC_KEY").unwrap_or_default();
    let poll_interval = Duration::from_secs(2);
    let max_attempts = (max_wait_minutes * 60) / 3;
    let mut attempts = 0;

    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(120))
        .build()
        .map_err(|e| format!("Failed to build HTTP client: {}", e))?;

    while attempts < max_attempts {
        attempts += 1;

        match client
            .get(&status_endpoint)
            .header("x-api-key", &api_key)
            .query(&[("phone", &user_phone)])
            .send()
            .await
        {
            Ok(res) if res.status().is_success() => {
                if let Ok(response) = res.json::<WebhookStatusResponse>().await {
                    if response.success {
                        if let Some(status_data) = response.data {
                            let status_lower = status_data.status.to_lowercase();

                            if status_lower == "completed" || status_lower == "successful" {
                                let completed_at = status_data.last_updated;
                                let duration = completed_at.signed_duration_since(initiated_at);
                                let minutes = duration.num_minutes();
                                let seconds = duration.num_seconds() % 60;

                                let time_taken = if minutes > 0 {
                                    format!("{} min {} sec", minutes, seconds)
                                } else {
                                    format!("{} seconds", seconds)
                                };

                                let success_msg = format!(
                                    "✅ *Withdrawal Completed Successfully! 🎉*\n\n\
                                    Funds deposited to your bank account:\n\n\
                                    💰 *Amount:* {:.2} {}\n\
                                    🏦 *Bank:* {}\n\
                                    👤 *Account Name:* {}\n\n\
                                    🔢 *Reference:* {}\n\n\
                                    ⏱️ *Withdrawal processed in:* {}\n\n\
                                    📅 *Completed at:* {}\n\n\
                                    Thank you for using KharonPay!",
                                    status_data.amount.unwrap_or(0.0),
                                    status_data.currency.as_deref().unwrap_or(""),
                                    bank_name,
                                    account_name,
                                    status_data.reference,
                                    time_taken,
                                    completed_at.format("%Y-%m-%d %H:%M:%S")
                                );

                                send_twilio_message(&user_phone, &success_msg).await;

                                println!(
                                    "Transaction {} completed in {} and notification sent",
                                    reference, time_taken
                                );

                                return Ok(());
                            }

                            if status_lower == "failed" || status_lower == "cancelled" {
                                let failure_msg = format!(
                                    "❌ *Withdrawal Failed*\n\n\
                                    Unfortunately, your withdrawal could not be completed.\n\n\
                                    🔢 **Reference:** {}\n\
                                    📅 **Status:** {}\n\n\
                                    Please contact support for assistance.",
                                    status_data.reference, status_data.status
                                );

                                send_twilio_message(&user_phone, &failure_msg).await;
                                return Err(format!("Transaction failed: {}", status_data.status));
                            }
                        }
                    }
                }
            }
            _ => {}
        }

        if attempts < max_attempts {
            sleep(poll_interval).await;
        }
    }

    Err(format!(
        "Polling timed out after {} minutes",
        max_wait_minutes
    ))
}

fn start_transaction_polling_task(
    reference: String,
    user_phone: String,
    bank_name: String,
    account_name: String,
    initiated_at: DateTime<Utc>,
) {
    tokio::spawn(async move {
        let _ = poll_and_notify_on_completion(
            reference,
            user_phone,
            bank_name,
            account_name,
            initiated_at,
            30,
        )
        .await;
    });
}

fn clear_session(session: &mut UserSessions) {
    session.state = UserState::Initial;
    session.pending_amount = None;
    session.pending_currency = None;
    session.pending_bank_verification = None;
    session.pending_bank_details = None;
}

async fn send_twilio_message(to: &str, message: &str) {
    let account_sid = std::env::var("T_ACCOUNT_SID").expect("T_ACCOUNT_SID must be set");
    let auth_token = std::env::var("T_AUTH_TOKEN").expect("T_AUTH_TOKEN must be set");
    let from_number = std::env::var("T_WHATSAPP_NUMBER").expect("T_WHATSAPP_NUMBER must be set");
    let url = std::env::var("T_API_URL").expect("T_API_URL must be set");

    let to_whatsapp = if to.starts_with("whatsapp:") {
        to.to_string()
    } else if to.starts_with("+") {
        format!("whatsapp:{}", to)
    } else {
        format!("whatsapp:+{}", to)
    };

    let auth_string = format!("{}:{}", account_sid, auth_token);
    let auth_encoded = Engine.encode(auth_string);

    let mut form_data = HashMap::new();
    form_data.insert("From", from_number.as_str());
    form_data.insert("To", &to_whatsapp);
    form_data.insert("Body", message);

    let client = reqwest::Client::new();
    let response = client
        .post(&url)
        .header("Authorization", format!("Basic {}", auth_encoded))
        .form(&form_data)
        .send()
        .await;

    if let Ok(resp) = response {
        if !resp.status().is_success() {
            eprintln!("Failed to send message: {}", resp.status());
        }
    }
}
