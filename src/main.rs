mod db;
mod models;
mod routes;
mod state;

use axum::{
    routing::{get, post},
    Router,
};
use std::net::SocketAddr;
use tower_http::cors::{Any, CorsLayer};
use tower_http::trace::TraceLayer;
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::from_default_env()
                .add_directive("engage_server=debug".parse().unwrap()),
        )
        .init();

    let db_path = std::env::var("DATABASE_PATH").unwrap_or_else(|_| "engage-server.db".into());
    let conn = db::open(std::path::Path::new(&db_path)).expect("failed to open database");
    let app_state = state::AppState::new(conn);

    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    let app = Router::new()
        // Key server endpoints
        .route("/api/register", post(routes::keys::register))
        .route("/api/keys/{user_id}", get(routes::keys::get_prekey_bundle))
        .route("/api/keys/{user_id}/prekeys", post(routes::keys::upload_prekeys))
        // Message relay endpoints
        .route("/api/messages", post(routes::messages::send_message))
        .route("/api/messages/{user_id}", get(routes::messages::fetch_messages))
        // WebSocket real-time delivery
        .route("/ws/{user_id}", get(routes::ws::ws_handler))
        .layer(cors)
        .layer(TraceLayer::new_for_http())
        .with_state(app_state);

    let port: u16 = std::env::var("PORT")
        .ok()
        .and_then(|p| p.parse().ok())
        .unwrap_or(3000);
    let addr = SocketAddr::from(([0, 0, 0, 0], port));
    tracing::info!("engage-server listening on {addr}");

    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    axum::serve(listener, app).await.unwrap();
}
