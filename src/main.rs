use actix_web::{App, HttpResponse, HttpServer, middleware::Logger, web};
use std::collections::HashMap;

use crate::server::{SessionMap, handle_twilio_webhook, health_check};

mod model;
mod server;

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    dotenv::dotenv().ok();
    env_logger::init();

    let _account_sid =
        std::env::var("TWILIO_ACCOUNT_SID").expect("TWILIO_ACCOUNT_SID must be set in .env file");
    let _auth_token =
        std::env::var("TWILIO_AUTH_TOKEN").expect("TWILIO_AUTH_TOKEN must be set in .env file");
    let _whatsapp_number = std::env::var("TWILIO_WHATSAPP_NUMBER")
        .expect("TWILIO_WHATSAPP_NUMBER must be set in .env file");
    let _api_url = std::env::var("TWILIO_API_URL").expect("TWILIO_API_URL must be set in .env file");

    let sessions: web::Data<std::sync::Mutex<SessionMap>> =
        web::Data::new(std::sync::Mutex::new(HashMap::new()));

    println!("🚀 Kharon Pay Twilio WhatsApp Server starting on port 6500");
    println!("📱 Webhook URL: http://localhost:6500/webhook");

    HttpServer::new(move || {
        App::new()
            .app_data(sessions.clone())
            .wrap(Logger::default())
            .route("/webhook", web::post().to(handle_twilio_webhook))
            .route("/health", web::get().to(health_check))
            .route(
                "/",
                web::get().to(|| async {
                    HttpResponse::Ok().json(serde_json::json!({
                        "message": "Kharon Pay WhatsApp Bot API",
                        "status": "running",
                        "webhook": "/webhook",
                    }))
                }),
            )
    })
    .bind("0.0.0.0:6500")?
    .run()
    .await
}
