use askama::Template;
use axum::response::{Html, IntoResponse};
use axum::{
    Json,
    extract::{Query, State},
    http::StatusCode,
};
use chrono::Utc;
use std::env;
use std::sync::Arc;
use validator::Validate;

use crate::AppState;
use crate::models::{FloodDisplay, SignUpRequest, User, VerifyParams};
use crate::tides::{FLOOD_THRESHOLD_FT, FORECAST_DAYS, get_flood_predictions};

#[derive(Template)]
#[template(path = "index.html")]
pub struct IndexTemplate {
    pub predictions: Vec<FloodDisplay>,
    pub forecast_days: i64,
    pub flood_threshold: f64,
}

pub async fn home_handler(State(state): State<Arc<AppState>>) -> impl axum::response::IntoResponse {
    let now = Utc::now();

    let predictions = match get_flood_predictions(&state.pool, now).await {
        Ok(preds) => preds,
        Err(e) => {
            eprintln!("Error fetching predictions: {}", e);
            Vec::new()
        }
    };

    let template = IndexTemplate {
        predictions,
        forecast_days: FORECAST_DAYS,
        flood_threshold: FLOOD_THRESHOLD_FT,
    };

    match template.render() {
        Ok(html) => Html(html).into_response(),
        Err(_) => (
            axum::http::StatusCode::INTERNAL_SERVER_ERROR,
            "Template Error",
        )
            .into_response(),
    }
}

pub async fn sign_up_handler(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<SignUpRequest>,
) -> Result<(StatusCode, String), (StatusCode, String)> {
    if let Err(_) = payload.validate() {
        return Err((
            StatusCode::BAD_REQUEST,
            "Please provide a valid email address.".to_string(),
        ));
    }

    let user = User::new(payload.email.into());

    let result = sqlx::query!(
        r#"
        INSERT INTO users (id, email, is_verified, verification_token, is_subscribed)
        VALUES (?, ?, ?, ?, ?)
        ON CONFLICT(email) DO UPDATE
        SET verification_token = excluded.verification_token, updated_at = CURRENT_TIMESTAMP
        WHERE users.is_verified = 0;
        "#,
        user.id,
        user.email,
        user.is_verified,
        user.verification_token,
        user.is_subscribed
    )
    .execute(&state.pool)
    .await;

    match result {
        Ok(res) => {
            if res.rows_affected() == 0 {
                // Email already exists and is verified
                return Err((
                    StatusCode::CONFLICT,
                    "Email already registered and verified".to_string(),
                ));
            }

            let base_url =
                env::var("BASE_URL").unwrap_or_else(|_| "http://127.0.0.1:3000".to_string());
            let validation_link = format!("{}/verify?token={}", base_url, user.verification_token);

            match state
                .mailer
                .send_verification_email(&user.email, &validation_link, &state.domain)
                .await
            {
                Ok(_) => Ok((StatusCode::OK, "Verification email sent!".to_string())),
                Err(e) => {
                    eprintln!("Mailgun error during verification: {:?}", e);

                    Err((
                        StatusCode::INTERNAL_SERVER_ERROR,
                        "Failed to add to mailing list.".to_string(),
                    ))
                }
            }
        }
        Err(e) => {
            eprintln!("Database error: {:?}", e);
            Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                "Internal server error".to_string(),
            ))
        }
    }
}

pub async fn verify_handler(
    State(state): State<Arc<AppState>>,
    Query(params): Query<VerifyParams>,
) -> Result<(StatusCode, String), (StatusCode, String)> {
    let result = sqlx::query!(
        r#"
        UPDATE users
        SET is_verified = 1, updated_at = CURRENT_TIMESTAMP
        WHERE verification_token = ? AND is_verified = 0
        RETURNING email;
        "#,
        params.token
    )
    .fetch_optional(&state.pool)
    .await;

    match result {
        Ok(None) => {
            return Err((
                StatusCode::BAD_REQUEST,
                "Invalid or already used verification token".to_string(),
            ));
        }

        Ok(Some(record)) => {
            match state
                .mailer
                .add_to_list(&state.mailing_list_id, &state.domain, &record.email)
                .await
            {
                Ok(_) => {
                    let update_result = sqlx::query!(
                        r#"
                        UPDATE users
                        SET is_subscribed = 1, updated_at = CURRENT_TIMESTAMP
                        WHERE email = ?;
                        "#,
                        record.email
                    )
                    .execute(&state.pool)
                    .await;

                    match update_result {
                        Ok(_) => Ok((
                            StatusCode::OK,
                            "Email verified and added to list!".to_string(),
                        )),
                        Err(e) => {
                            eprintln!("Database error during subscription update: {:?}", e);
                            Err((
                                StatusCode::INTERNAL_SERVER_ERROR,
                                "Failed to update subscription status.".to_string(),
                            ))
                        }
                    }
                }
                Err(e) => {
                    eprintln!("Mailgun error during verification: {:?}", e);

                    Err((
                        StatusCode::INTERNAL_SERVER_ERROR,
                        "Failed to add to mailing list.".to_string(),
                    ))
                }
            }
        }
        Err(e) => {
            eprintln!("Database error: {:?}", e);
            Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                "Internal server error".to_string(),
            ))
        }
    }
}

pub async fn fallback_handler(
    State(_): State<Arc<AppState>>,
    Json(_): Json<SignUpRequest>,
) -> Result<(StatusCode, String), (StatusCode, String)> {
    Err((StatusCode::NOT_FOUND, "Not Found".to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{Json, extract::State};
    use sqlx::sqlite::SqlitePoolOptions;

    async fn setup_test_db() -> SqlitePool {
        let pool = SqlitePoolOptions::new()
            .connect("sqlite::memory:")
            .await
            .unwrap();
        sqlx::migrate!().run(&pool).await.unwrap();
        pool
    }

    async fn insert_test_user(pool: &SqlitePool, email: &str, is_verified: bool) -> String {
        let user_id = Uuid::new_v4().to_string();
        let verification_token = Uuid::new_v4().to_string();
        let is_verified_int = if is_verified { 1 } else { 0 };

        sqlx::query!(
            r#"
            INSERT INTO users (id, email, verification_token, is_verified)
            VALUES (?, ?, ?, ?);
            "#,
            user_id,
            email,
            verification_token,
            is_verified_int
        )
        .execute(pool)
        .await
        .unwrap();

        verification_token
    }

    #[tokio::test]
    async fn sign_up_success() {
        let pool = setup_test_db().await;
        let payload = Json(SignUpRequest {
            email: "some@email.com".into(),
        });

        let (status, body) = sign_up_handler(State(pool), payload).await.unwrap();

        assert_eq!(status, StatusCode::OK);
        assert!(body.contains("/verify?token="));
    }

    #[tokio::test]
    async fn sign_up_existing_user() {
        let pool = setup_test_db().await;

        // Insert a verified user
        insert_test_user(&pool, "some@email.com", true).await;

        let payload = Json(SignUpRequest {
            email: "some@email.com".into(),
        });

        if let Err((status, body)) = sign_up_handler(State(pool), payload).await {
            assert_eq!(status, StatusCode::CONFLICT);
            assert_eq!(body, "Email already registered and verified".to_string());
        } else {
            panic!("Expected conflict error for existing verified user");
        }
    }

    #[tokio::test]
    async fn verify_success() {
        let pool = setup_test_db().await;

        let verification_token = insert_test_user(&pool, "some@email.com", false).await;

        let params = Query(VerifyParams {
            token: verification_token,
        });
        let (status, body) = verify_handler(State(pool), params).await.unwrap();
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body, "Email verified successfully".to_string());
    }

    #[tokio::test]
    async fn verify_invalid_token() {
        let pool = setup_test_db().await;
        let params = Query(VerifyParams {
            token: "invalid_token".to_string(),
        });
        if let Err((status, _)) = verify_handler(State(pool), params).await {
            assert_eq!(status, StatusCode::BAD_REQUEST);
        } else {
            panic!("Expected bad request error for invalid token");
        }
    }
}
