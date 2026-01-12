use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::{NoContext, Timestamp, Uuid};
use validator::Validate;

use hmac::{Hmac, Mac};
use sha2::Sha256;

type HmacSha256 = Hmac<Sha256>;

#[derive(Debug, Deserialize, Serialize, Validate)]
pub struct SignUpRequest {
    #[validate(email(message = "Invalid email format"))]
    pub email: String,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct VerifyParams {
    pub token: String,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct UnsubscribeParams {
    pub id: String,
    pub token: String,
}

#[derive(Debug, Serialize, Deserialize, FromRow, Default)]
pub struct User {
    pub id: String,
    pub email: String,
    pub is_verified: bool,
    pub verification_token: String,
    pub is_subscribed: bool,
}

impl User {
    pub fn new(email: String) -> Self {
        let timestamp: Timestamp = Timestamp::now(NoContext);
        let id = Uuid::new_v7(timestamp).to_string();
        let verification_token = Uuid::new_v4().to_string();

        User {
            id,
            email,
            is_verified: false,
            verification_token,
            is_subscribed: false,
        }
    }

    pub fn generate_unsubscribe_token(&self, secret: &str) -> String {
        let mut mac = HmacSha256::new_from_slice(secret.as_bytes()).unwrap();
        mac.update(self.id.as_bytes());
        let token_bytes = mac.finalize().into_bytes();
        hex::encode(token_bytes)
    }

    pub fn verify_unsubscribe_token(&self, token: &str, secret: &str) -> bool {
        let expected_token = self.generate_unsubscribe_token(secret);
        expected_token == token
    }
}

#[derive(Clone)]
pub struct FloodDisplay {
    pub datetime: String,
    pub height: String,
}
