mod auth;
mod db;
mod models;
mod routes;
mod state;

use axum::{middleware, routing::{get, post}, Router};
use std::net::SocketAddr;
use tower_http::cors::{Any, CorsLayer};
use tower_http::trace::TraceLayer;
use tracing_subscriber::EnvFilter;

use state::{AppState, OAuthConfig};

#[tokio::main]
async fn main() {
    dotenvy::dotenv().ok();

    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::from_default_env()
                .add_directive("engage_server=debug".parse().unwrap()),
        )
        .init();

    let db_path = std::env::var("DATABASE_PATH").unwrap_or_else(|_| "engage-server.db".into());
    let conn = db::open(std::path::Path::new(&db_path)).expect("failed to open database");

    let oauth = OAuthConfig {
        client_id: std::env::var("GOOGLE_CLIENT_ID").expect("GOOGLE_CLIENT_ID required"),
        client_secret: std::env::var("GOOGLE_CLIENT_SECRET").expect("GOOGLE_CLIENT_SECRET required"),
        redirect_uri: std::env::var("GOOGLE_REDIRECT_URI")
            .unwrap_or_else(|_| "http://localhost:3000/api/auth/google/callback".into()),
        jwt_secret: std::env::var("JWT_SECRET").expect("JWT_SECRET required"),
    };

    let app_state = AppState::new(conn, oauth);

    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    // Public routes — no auth required
    let public = Router::new()
        .route("/api/auth/google", get(routes::oauth::start))
        .route("/api/auth/google/callback", get(routes::oauth::callback));

    // Protected routes — JWT required
    let protected = Router::new()
        .route("/api/register", post(routes::keys::register))
        .route("/api/keys/{user_id}", get(routes::keys::get_prekey_bundle))
        .route("/api/keys/{user_id}/prekeys", post(routes::keys::upload_prekeys))
        .route("/api/keys/{user_id}/prekeys/count", get(routes::keys::opk_count))
        .route("/api/messages", post(routes::messages::send_message))
        .route("/api/messages/{user_id}", get(routes::messages::fetch_messages))
        // Group routes
        .route("/api/groups", post(routes::groups::create_group))
        .route("/api/groups", get(routes::groups::list_groups))
        .route("/api/groups/{id}", get(routes::groups::get_group))
        .route("/api/groups/{id}/members", post(routes::groups::add_member))
        .route("/api/groups/{id}/members/{user_id}", axum::routing::delete(routes::groups::remove_member))
        .route("/api/groups/{id}/messages", post(routes::groups::send_group_message))
        .route("/ws/{user_id}", get(routes::ws::ws_handler))
        .layer(middleware::from_fn_with_state(
            app_state.clone(),
            auth::require_auth,
        ));

    let app = public
        .merge(protected)
        .layer(cors)
        .layer(TraceLayer::new_for_http())
        .with_state(app_state);

    let port: u16 = std::env::var("PORT")
        .ok()
        .and_then(|p| p.parse().ok())
        .unwrap_or(3000);
    let addr = SocketAddr::from(([0, 0, 0, 0], port));
    tracing::info!("engage-server listening on {addr}");

    let listener = tokio::net::TcpListener::bind(addr).await.unwrap_or_else(|e| {
        tracing::error!("failed to bind to {addr}: {e} — port {port} already in use. Kill the existing process with: lsof -ti:{port} | xargs kill");
        std::process::exit(1);
    });
    axum::serve(listener, app).await.unwrap();
}
