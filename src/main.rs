use axum::{
    Router,
    extract::State,
    response::Html,
    routing::{get, post},
};
use dotenvy::dotenv;
use sqlx::sqlite::{
    SqliteConnectOptions, SqliteJournalMode, SqlitePool, SqlitePoolOptions, SqliteSynchronous,
};
use std::env;
use std::str::FromStr;
use std::sync::Arc;
use tower_http::trace::TraceLayer;
use tracing_subscriber;

mod handlers;
mod mail;
mod models;
mod tides;

use crate::handlers::{fallback_handler, home_handler, sign_up_handler, verify_handler};
use crate::mail::MailgunClient;
use crate::tides::update_tide_predictions;

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
}

struct AppState {
    mailer: MailgunClient,
    pool: SqlitePool,
    mailing_list_id: String,
    domain: String,
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
        Commands::Serve => serve(pool).await,
        Commands::Sync => update_tide_predictions(pool).await,
    }
}

async fn serve(pool: SqlitePool) -> Result<(), Box<dyn std::error::Error>> {
    println!("Starting server...");

    let domain = env::var("DOMAIN").expect("DOMAIN must be set");
    let mailgun_api_key = env::var("MAILGUN_API_KEY").expect("MAILGUN_API_KEY must be set");
    let mailing_list_id = env::var("MAILING_LIST_ID").expect("MAILING_LIST_ID must be set");

    let mailer = MailgunClient {
        client: reqwest::Client::new(),
        api_key: mailgun_api_key,
    };

    let app_state = Arc::new(AppState {
        mailer,
        pool,
        mailing_list_id,
        domain,
    });

    let app = Router::new()
        .route("/", get(home_handler))
        .route("/signup", post(sign_up_handler))
        .route("/verify", get(verify_handler))
        .fallback(fallback_handler)
        .layer(TraceLayer::new_for_http())
        .with_state(app_state);

    let listener = tokio::net::TcpListener::bind("127.0.0.1:3000").await?;
    println!("Server running on http://127.0.0.1:3000");
    axum::serve(listener, app).await?;

    Ok(())
}
