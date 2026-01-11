use askama::Template;
use axum::response::{Html, IntoResponse};
use axum::{
    Json,
    extract::{Query, State},
    http::{Method, StatusCode},
};
use chrono::Utc;
use std::sync::Arc;
use validator::Validate;

use crate::AppState;
use crate::models::{FloodDisplay, SignUpRequest, UnsubscribeParams, User, VerifyParams};
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
    if payload.validate().is_err() {
        return Err((
            StatusCode::BAD_REQUEST,
            "Please provide a valid email address.".to_string(),
        ));
    }

    let user = User::new(payload.email);

    let result = sqlx::query!(
        r#"
        INSERT INTO users (id, email, is_verified, verification_token, is_subscribed)
        VALUES (?, ?, ?, ?, ?)
        ON CONFLICT(email) DO UPDATE
        SET verification_token = excluded.verification_token, is_verified = 0, is_subscribed = 0
        WHERE users.is_verified = 0 OR users.is_subscribed = 0
        RETURNING id;
        "#,
        user.id,
        user.email,
        user.is_verified,
        user.verification_token,
        user.is_subscribed
    )
    .fetch_optional(&state.pool)
    .await;

    match result {
        Ok(None) => Err((
            StatusCode::CONFLICT,
            "Email already registered and verified".to_string(),
        )),
        Ok(Some(res)) => {
            let user = User { id: res.id, ..user };
            let validation_link = format!(
                "{}/verify?token={}",
                &state.base_url, user.verification_token
            );
            let unsubscribe_link = format!(
                "{}/unsubscribe?id={}&token={}",
                &state.base_url,
                user.id,
                user.generate_unsubscribe_token(&state.unsubscribe_secret)
            );
            match state
                .mailer
                .send_verification_email(&user, &validation_link, &unsubscribe_link)
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

#[derive(Template)]
#[template(path = "unsubscribe.html")]
pub struct UnsubscribeTemplate {
    pub user_id: String,
    pub token: String,
}

pub async fn unsubscribe_handler(
    method: Method,
    State(state): State<Arc<AppState>>,
    Query(params): Query<UnsubscribeParams>,
) -> impl IntoResponse {
    let user = User {
        id: params.id.clone(),
        ..Default::default()
    };

    if !user.verify_unsubscribe_token(&params.token, &state.unsubscribe_secret) {
        return (StatusCode::BAD_REQUEST, "Invalid unsubscribe token").into_response();
    }
    println!(
        "Unsubscribe request for user_id: {}, {}",
        params.id, params.token
    );
    match method {
        Method::GET => {
            let template = UnsubscribeTemplate {
                user_id: params.id,
                token: params.token,
            };
            Html(
                template
                    .render()
                    .unwrap_or_else(|_| "Template Error".into()),
            )
            .into_response()
        }
        Method::POST => {
            let result = sqlx::query!(
                r#"
                UPDATE users
                SET is_subscribed = 0
                WHERE id = ?;
                "#,
                params.id
            )
            .execute(&state.pool)
            .await;

            let (success, message) = match result {
                Ok(res) if res.rows_affected() > 0 => {
                    (true, "You have been successfully unsubscribed.".to_string())
                }
                Ok(_) => (false, "You are already unsubscribed.".to_string()),
                Err(e) => {
                    eprintln!("Database error: {:?}", e);
                    (
                        false,
                        "An internal error occurred. Please try again later.".to_string(),
                    )
                }
            };
            let verify_template = VerifyResultTemplate { success, message };
            match verify_template.render() {
                Ok(html) => Html(html).into_response(),
                Err(_) => (
                    axum::http::StatusCode::INTERNAL_SERVER_ERROR,
                    "Template Error",
                )
                    .into_response(),
            }
        }
        _ => (StatusCode::METHOD_NOT_ALLOWED, "Method not allowed").into_response(),
    }
}

#[derive(Template)]
#[template(path = "verify_result.html")]
pub struct VerifyResultTemplate {
    pub success: bool,
    pub message: String,
}

pub async fn verify_handler(
    State(state): State<Arc<AppState>>,
    Query(params): Query<VerifyParams>,
) -> impl axum::response::IntoResponse {
    let result = sqlx::query!(
        r#"
        UPDATE users
        SET is_verified = 1, is_subscribed = 1
        WHERE verification_token = ? AND is_verified = 0
        RETURNING email;
        "#,
        params.token
    )
    .fetch_optional(&state.pool)
    .await;

    let (success, message) = match result {
        Ok(None) => (
            false,
            "Invalid or already used verification token".to_string(),
        ),
        Ok(res) => (
            true,
            format!("Email: {} verified successfully", res.unwrap().email),
        ),
        Err(e) => {
            eprintln!("Database error: {:?}", e);
            (false, "Internal server error".to_string())
        }
    };

    let template = VerifyResultTemplate { success, message };

    match template.render() {
        Ok(html) => Html(html).into_response(),
        Err(_) => (
            axum::http::StatusCode::INTERNAL_SERVER_ERROR,
            "Template Error",
        )
            .into_response(),
    }
}

pub async fn fallback_handler(
    State(_): State<Arc<AppState>>,
    Json(_): Json<SignUpRequest>,
) -> Result<(StatusCode, String), (StatusCode, String)> {
    Err((StatusCode::NOT_FOUND, "Not Found".to_string()))
}
