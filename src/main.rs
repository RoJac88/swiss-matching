use std::{env, net::SocketAddr};

use axum::{Router, extract::FromRef};
use reqwest::Client;
use sqlx::{SqlitePool, sqlite::SqlitePoolOptions};
use tokio::net::TcpListener;
use tower_http::{cors::CorsLayer, trace::TraceLayer};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

use crate::{
    auth::admin::create_administrator,
    handlers::{players, tournaments},
};

mod auth;
mod errors;
mod handlers;
mod models;
mod payloads;
mod repositories;
mod responses;
mod services;

#[derive(Clone)]
struct AppState {
    pool: SqlitePool,
    client: reqwest::Client,
}

impl FromRef<AppState> for SqlitePool {
    fn from_ref(input: &AppState) -> Self {
        input.pool.clone()
    }
}

impl FromRef<AppState> for reqwest::Client {
    fn from_ref(input: &AppState) -> Self {
        input.client.clone()
    }
}

#[tokio::main]
async fn main() {
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| {
                "swiss_matching=debug,tower_http=debug,axum::rejection=trace".into()
            }),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();
    let db_url = env::var("DATABASE_URL").unwrap();
    let pool = SqlitePoolOptions::new().connect(&db_url).await.unwrap();
    create_administrator(&pool).await;
    let client = Client::builder()
        .user_agent("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/124.0.0.0 Safari/537.36")
        .build()
        .unwrap();
    let state = AppState { pool, client };
    let addr = SocketAddr::from(([127, 0, 0, 1], 3001));
    let listener = TcpListener::bind(addr).await.unwrap();
    tracing::info!("listening on {}", addr);
    let app = Router::new()
        .nest("/players", players::routes(state.clone()))
        .nest("/tournaments", tournaments::routes(state.clone()))
        .merge(handlers::auth::routes(state.clone()))
        .layer(TraceLayer::new_for_http())
        .layer(CorsLayer::very_permissive());
    axum::serve(listener, app).await.unwrap();
}
