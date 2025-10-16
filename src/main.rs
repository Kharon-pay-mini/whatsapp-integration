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
        std::env::var("T_ACCOUNT_SID").expect("T_ACCOUNT_SID must be set in .env file");
    let _auth_token =
        std::env::var("T_AUTH_TOKEN").expect("T_AUTH_TOKEN must be set in .env file");
    let _whatsapp_number = std::env::var("T_WHATSAPP_NUMBER")
        .expect("T_WHATSAPP_NUMBER must be set in .env file");
    let _api_url = std::env::var("T_API_URL").expect("T_API_URL must be set in .env file");

    let sessions: web::Data<std::sync::Mutex<SessionMap>> =
        web::Data::new(std::sync::Mutex::new(HashMap::new()));

    println!("🚀 Kharon Pay WhatsApp Server starting on port 6500");
    
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
