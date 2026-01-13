use axum::{
    Router,
    routing::{any, get, post},
};
use dotenvy::dotenv;
use sqlx::sqlite::{
    SqliteConnectOptions, SqliteJournalMode, SqlitePool, SqlitePoolOptions, SqliteSynchronous,
};
use std::env;
use std::str::FromStr;
use std::sync::Arc;
use tower_http::services::ServeDir;
use tower_http::trace::TraceLayer;

mod handlers;
mod mail;
mod models;
mod tides;

use crate::handlers::{
    fallback_handler, home_handler, privacy_policy_handler, sign_up_handler, unsubscribe_handler,
    verify_handler,
};
use crate::mail::SmtpClient;
use crate::models::User;
use crate::tides::{get_flood_predictions, update_tide_predictions};
use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "mv-sausalito-bikepath-flood-alert")]
#[command(about = "Flood alerts for the MV-Sausalito bike path", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    Serve,
    Sync,
    Notify,
}

struct AppState {
    mailer: SmtpClient,
    pool: SqlitePool,
    base_url: String,
    unsubscribe_secret: String,
}

impl AppState {
    fn from_pool(pool: SqlitePool) -> Self {
        let base_url = env::var("BASE_URL").expect("BASE_URL must be set");
        let unsubscribe_secret =
            env::var("UNSUBSCRIBE_SECRET").expect("UNSUBSCRIBE_SECRET must be set");

        let mailer = SmtpClient::new(
            env::var("SMTP_SERVER").expect("SMTP_SERVER must be set"),
            env::var("SMTP_PORT")
                .expect("SMTP_PORT must be set")
                .parse()
                .expect("SMTP_PORT must be a valid u16"),
            env::var("SMTP_USER").expect("SMTP_USER must be set"),
            env::var("SMTP_PASSWORD").expect("SMTP_PASSWORD must be set"),
            env::var("SMTP_FROM").expect("SMTP_FROM must be set"),
            base_url.clone(),
        );

        AppState {
            mailer,
            pool,
            base_url,
            unsubscribe_secret,
        }
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    dotenv().ok();

    let cli = Cli::parse();

    tracing_subscriber::fmt()
        .with_env_filter("mill_valley_sausalito_bikepath_flood_alert=debug,tower_http=debug")
        .init();

    let database_url = env::var("DATABASE_URL").expect("DATABASE_URL must be set");

    let opts = SqliteConnectOptions::from_str(&database_url)?
        .journal_mode(SqliteJournalMode::Wal)
        .synchronous(SqliteSynchronous::Normal);

    let pool = SqlitePoolOptions::new()
        .max_connections(5)
        .connect_with(opts)
        .await?;

    sqlx::migrate!().run(&pool).await?;

    println!("Database migrations applied successfully.");

    match cli.command {
        Commands::Sync => update_tide_predictions(pool).await,
        Commands::Serve => serve(pool).await,
        Commands::Notify => check_and_send_notifications(pool).await,
    }
}

async fn serve(pool: SqlitePool) -> Result<(), Box<dyn std::error::Error>> {
    println!("Starting server...");

    let app_state = Arc::new(AppState::from_pool(pool));

    let app = Router::new()
        .route("/", get(home_handler))
        .route("/signup", post(sign_up_handler))
        .route("/verify", get(verify_handler))
        .route("/unsubscribe", any(unsubscribe_handler))
        .route("/privacy", get(privacy_policy_handler))
        .fallback(fallback_handler)
        .layer(TraceLayer::new_for_http())
        .with_state(app_state)
        .nest_service("/assets", ServeDir::new("assets"));

    let host = env::var("HOST").unwrap_or_else(|_| "127.0.0.1".to_string());
    let addr = format!("{}:3000", host);
    let listener = tokio::net::TcpListener::bind(&addr).await?;
    println!("Server running on http://{}", addr);
    axum::serve(listener, app).await?;

    Ok(())
}

async fn check_and_send_notifications(pool: SqlitePool) -> Result<(), Box<dyn std::error::Error>> {
    println!("Checking for flood predictions and sending notifications...");

    let base_url = env::var("BASE_URL").expect("BASE_URL must be set");
    let unsubscribe_secret =
        env::var("UNSUBSCRIBE_SECRET").expect("UNSUBSCRIBE_SECRET must be set");

    let predictions = get_flood_predictions(&pool, chrono::Utc::now()).await?;
    if predictions.is_empty() {
        println!("No flood predictions found. No email notifications to send.");
        return Ok(());
    }
    println!(
        "Found {} flood predictions. Sending email notifications...",
        predictions.len()
    );

    let recipients: Vec<User> = sqlx::query!(
        r#"
        SELECT id, email FROM mailing_list
        "#
    )
    .fetch_all(&pool)
    .await?
    .into_iter()
    .map(|record| User {
        id: record.id,
        email: record.email,
        ..Default::default()
    })
    .collect();
    println!("Sending emails to: {:?}", recipients);
    let unsubscribe_links: Vec<String> = recipients
        .iter()
        .map(|user| {
            format!(
                "{}/unsubscribe?id={}&token={}",
                &base_url,
                &user.id,
                &user.generate_unsubscribe_token(&unsubscribe_secret)
            )
        })
        .collect();

    let app_state = Arc::new(AppState::from_pool(pool));

    app_state
        .mailer
        .send_list_notification_email(predictions, recipients, unsubscribe_links)
        .await?;

    Ok(())
}
