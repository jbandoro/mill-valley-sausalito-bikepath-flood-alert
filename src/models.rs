use chrono::NaiveDateTime;
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

impl FloodDisplay {
    pub fn new(prediction_time: NaiveDateTime, height_ft: f64) -> Self {
        FloodDisplay {
            datetime: prediction_time.format("%A, %B %-d at %-I:%M%p").to_string(),
            height: format!("{:.2}", height_ft),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::NaiveDate;

    #[test]
    fn test_flood_display_formatting() {
        let dt = NaiveDate::from_ymd_opt(2023, 10, 5)
            .unwrap()
            .and_hms_opt(14, 30, 0)
            .unwrap();
        let display = FloodDisplay::new(dt, 6.789);

        assert_eq!(display.datetime, "Thursday, October 5 at 2:30PM");
        assert_eq!(display.height, "6.79");
    }

    #[test]
    fn test_user_defaults() {
        let email = "test@example.com".to_string();
        let user = User::new(email.clone());

        assert_eq!(user.email, email);
        assert!(!user.id.is_empty(), "ID should be generated");
        assert!(
            !user.verification_token.is_empty(),
            "Token should be generated"
        );
        assert_eq!(user.is_verified, false);
        assert_eq!(user.is_subscribed, false);
    }

    #[test]
    fn test_user_token_generation_and_verification() {
        let user = User::new("test@example.com".to_string());
        let secret = "super-secret-key";

        let token = user.generate_unsubscribe_token(secret);
        assert!(!token.is_empty(), "Token should not be empty");

        // Verify with correct secret
        assert!(
            user.verify_unsubscribe_token(&token, secret),
            "Token verification failed with correct secret"
        );

        // Verify with incorrect secret
        assert!(
            !user.verify_unsubscribe_token(&token, "wrong-secret"),
            "Token verification succeeded with wrong secret"
        );

        // Verify with incorrect token
        assert!(
            !user.verify_unsubscribe_token("wrong-token", secret),
            "Token verification succeeded with wrong token"
        );

        // Verify uniqueness (different users -> different tokens)
        let other_user = User::new("other@example.com".to_string());
        let other_token = other_user.generate_unsubscribe_token(secret);
        assert_ne!(
            token, other_token,
            "Different users should have different tokens"
        );
    }
}
