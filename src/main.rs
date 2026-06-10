// SPDX-License-Identifier: AGPL-3.0-only
// Copyright (C) 2024-2026 Farid Guzman <https://github.com/faridguzman91>

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
        // @faridguzman: Optional — set to enable TURN credentials endpoint.
        // Generate with: openssl rand -hex 32
        turn_secret: std::env::var("TURN_SECRET").ok(),
        nextcloud: {
            // @faridguzman: All three vars must be set to enable Nextcloud auth.
            // NEXTCLOUD_SERVER_NAME is optional — defaults to the host part of the URL.
            match (
                std::env::var("NEXTCLOUD_URL"),
                std::env::var("NEXTCLOUD_CLIENT_ID"),
                std::env::var("NEXTCLOUD_CLIENT_SECRET"),
            ) {
                (Ok(url), Ok(cid), Ok(csec)) => {
                    let name = std::env::var("NEXTCLOUD_SERVER_NAME")
                        .unwrap_or_else(|_| {
                            url::Url::parse(&url)
                                .ok()
                                .and_then(|u| u.host_str().map(String::from))
                                .unwrap_or_else(|| "Nextcloud".into())
                        });
                    let redirect = std::env::var("NEXTCLOUD_REDIRECT_URI")
                        .unwrap_or_else(|_| {
                            format!("http://localhost:3000/api/auth/nextcloud/callback")
                        });
                    Some(state::NextcloudConfig {
                        server_url: url,
                        server_name: name,
                        client_id: cid,
                        client_secret: csec,
                        redirect_uri: redirect,
                    })
                }
                _ => None,
            }
        },
    };

    let app_state = AppState::new(conn, oauth);

    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    // Public routes — no auth required
    let public = Router::new()
        .route("/api/auth/google", get(routes::oauth::start))
        .route("/api/auth/google/callback", get(routes::oauth::callback))
        // @faridguzman: Nextcloud OAuth — routes are public (no JWT yet at login time)
        .route("/api/auth/nextcloud", get(routes::nextcloud::start))
        .route("/api/auth/nextcloud/callback", get(routes::nextcloud::callback))
        // @faridguzman: Providers discovery — tells the client which auth methods are enabled.
        // Called from LoginView before the user has a JWT.
        .route("/api/auth/providers", get(routes::nextcloud::providers))
        // @faridguzman: Invite redemption is public so the recipient does not need
        // an account yet when they tap the link.
        .route("/api/invites/{token}", get(routes::invites::redeem_invite));

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
        // @faridguzman: Invite creation — authenticated so only registered users can issue links.
        .route("/api/invites", post(routes::invites::create_invite))
        // @faridguzman: Short-lived TURN credentials for WebRTC NAT traversal.
        .route("/api/turn-credentials", get(routes::turn::turn_credentials))
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

    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    axum::serve(listener, app).await.unwrap();
}
