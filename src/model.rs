use serde::{Deserialize, Serialize};

#[derive(Debug, Clone)]
pub struct UserSessions {
    pub phone: String,
    pub state: UserState,
    pub account_id: Option<String>,
    pub pending_amount: Option<f64>,
    pub pending_currency: Option<String>,
    pub controller_address: Option<String>,
    pub pending_bank_details: Option<BankDetails>,
    pub pending_bank_verification: Option<BankVerificationResponse>,
}

#[derive(Debug, Clone)]
pub enum UserState {
    Initial,
    AccountCreation,
    BankDetailsEntry,
    OfframpConfirmation,
    BankDetailsConfirmation,
    SavedBankConfirmation,
}

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct BankVerificationResponse {
    pub bank_name: String,
    pub account_number: String,
    pub account_name: String,
    pub bank_code: String,
}

#[derive(Deserialize, Debug)]
pub struct CreateControllerData {
    pub controller_address: String,
    pub username: String,
    pub session_id: String,
    pub session_options: serde_json::Value,
}

#[derive(Deserialize, Debug)]
pub struct CreateControllerAPIResponse {
    pub success: String,
    pub message: String,
    pub data: CreateControllerData,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BankDetails {
    pub bank_details_id: String,
    pub bank_name: String,
    #[serde(rename = "bank_account_number")]
    pub account_number: String,
    pub account_name: String,
}

#[derive(Debug, serde::Deserialize)]
pub struct BankListResponse {
    pub status: String,
    pub data: BankListResponseData,
}

#[derive(Debug, serde::Deserialize)]
pub struct BankListResponseData {
    pub banks: Vec<BankDetails>,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct ReceivePaymentRequest {
    pub token: String,
    pub amount: String,
    pub reference: String,
    pub phone: String,
}

#[derive(Debug, Deserialize)]
pub struct DisbursementDetails {
    pub account_name: String,
    pub account_number: String,
    pub bank_name: String,
    pub bank_code: String,
    pub amount: f64, 
    pub currency: String,
    pub crypto_tx_hash: String,
}

#[derive(Debug, Deserialize)]
pub struct InitDisbursementResponse {
    pub success: bool,
    pub message: String,
    pub reference: String,
    pub data: Option<DisbursementDetails>,
    pub error: Option<String>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct TransactionStatus {
    pub transaction_id: String,
    pub reference: String,
    pub status: String,
    pub amount: Option<f64>,
    pub currency: Option<String>,
    pub last_updated: chrono::DateTime<chrono::Utc>,
    pub metadata: Option<serde_json::Value>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct WebhookStatusResponse {
    pub success: bool,
    pub data: Option<TransactionStatus>,
    pub message: String,
}