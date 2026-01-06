use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::{NoContext, Timestamp, Uuid};
use validator::Validate;

#[derive(Debug, Deserialize, Serialize, Validate)]
pub struct SignUpRequest {
    #[validate(email(message = "Invalid email format"))]
    pub email: String,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct VerifyParams {
    pub token: String,
}

#[derive(Debug, Serialize, Deserialize, FromRow)]
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
}

pub struct FloodDisplay {
    pub datetime: String,
    pub height: String,
}
