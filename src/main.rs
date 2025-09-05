use axum::{routing::get, Router};
use serde_json::{json, Value};
use tower_http::{cors::CorsLayer, trace::TraceLayer};

mod api;
mod auth;
mod config;
mod database;
mod error;
mod filter;
mod handlers;
mod observer;

#[tokio::main]
async fn main() {
    // Load .env if present so cargo run picks up DATABASE_URL, MONK_TENANT_DB, etc.
    let _ = dotenvy::dotenv();

    // Initialize configuration (this loads the config singleton)
    let config = crate::config::config();
    tracing::info!("Starting Monk API in {:?} mode", config.environment);

    tracing_subscriber::fmt::init();

    let app = app();

    // Allow tests or deployments to override port via env
    let port = std::env::var("MONK_API_PORT")
        .ok()
        .or_else(|| std::env::var("PORT").ok())
        .and_then(|s| s.parse::<u16>().ok())
        .unwrap_or(3000);

    let bind_addr = format!("0.0.0.0:{}", port);
    let listener = tokio::net::TcpListener::bind(&bind_addr)
        .await
        .unwrap_or_else(|e| panic!("failed to bind {}: {}", bind_addr, e));

    println!("🚀 Monk API Rust server listening on http://{}", bind_addr);

    axum::serve(listener, app).await.expect("server");
}

fn app() -> Router {
    Router::new()
        // Public
        .route("/", get(root))
        .route("/health", get(health))
        // Public auth routes
        .merge(auth_public_routes())
        // Protected API (auth skipped for MVP)
        .merge(data_routes())
        .merge(find_routes())
        .merge(meta_routes())
        .merge(auth_routes())
        // Global middleware
        .layer(CorsLayer::permissive())
        .layer(TraceLayer::new_for_http())
}

fn auth_public_routes() -> Router {
    use axum::routing::{delete, post, put};
    use handlers::public::auth;

    Router::new()
        // Session management with tenant and user in path
        .route("/auth/login/:tenant/:user", post(auth::session_login))
        .route("/auth/refresh/:tenant/:user", post(auth::session_refresh))
        // User management
        .route("/auth/register", post(auth::user_register))
        .route("/auth/activate", put(auth::user_activate))
        .route("/auth/user", delete(auth::user_delete))
}

fn auth_routes() -> Router {
    use axum::routing::{delete, post, put};
    use handlers::protected::auth;

    Router::new()
        // Session management for authenticated users
        .route("/api/auth/whoami", get(auth::session_whoami))
        .route("/api/auth/sudo", post(auth::session_sudo))
        .route("/api/auth/session/refresh", put(auth::session_refresh))
        .route("/api/auth/session", delete(auth::session_logout))
}

fn data_routes() -> Router {
    use axum::routing::{delete, patch, post, put};
    use handlers::protected::data;

    Router::new()
        // Schema-level operations (collection)
        .route(
            "/api/data/:schema",
            get(data::schema_get)
                .post(data::schema_post)
                .put(data::schema_put)
                .patch(data::schema_patch)
                .delete(data::schema_delete),
        )
        // Record-level operations (individual)
        .route(
            "/api/data/:schema/:id",
            get(data::record_get)
                .put(data::record_put)
                .patch(data::record_patch)
                .delete(data::record_delete),
        )
        // Record restore endpoint
        .route("/api/data/:schema/:id/restore", post(data::record_restore))
}

fn find_routes() -> Router {
    use axum::routing::{delete, post};
    use handlers::protected::find;

    Router::new()
        // Find/search operations with filters
        .route("/api/find/:schema", post(find::find_post).delete(find::find_delete))
}

fn meta_routes() -> Router {
    use axum::routing::{delete, post, put};
    use handlers::protected::meta;

    Router::new()
        // Schema definition management
        .route(
            "/api/meta/:schema",
            get(meta::schema_get)
                .post(meta::schema_post)
                .put(meta::schema_put)
                .delete(meta::schema_delete),
        )
}

async fn root() -> axum::response::Json<Value> {
    let version = env!("CARGO_PKG_VERSION");

    axum::response::Json(json!({
        "success": true,
        "data": {
            "name": "Monk API (Rust)",
            "version": version,
            "description": "Lightweight PaaS backend API built with Rust (Axum)",
            "endpoints": {
                "home": "/ (public)",
                "public_auth": "/auth/login/:tenant/:user, /auth/refresh/:tenant/:user (public - token acquisition)",
                "docs": "/docs[/:api] (public)",
                "auth": "/api/auth/* (protected - user management)",
                "meta": "/api/meta/:schema (protected)",
                "data": "/api/data/:schema[/:record] (protected)",
                "find": "/api/find/:schema (protected)",
                "bulk": "/api/bulk (protected)",
                "file": "/api/file/* (protected)",
                "acls": "/api/acls/:schema/:record (protected)",
                "root": "/api/root/* (restricted, requires sudo or localhost)",
            },
            "documentation": {
                "home": ["/README.md"],
                "auth": ["/docs/auth", "/docs/public-auth"],
                "meta": ["/docs/meta"],
                "data": ["/docs/data"],
                "find": ["/docs/find"],
                "bulk": ["/docs/bulk"],
                "file": ["/docs/file"],
                "acls": ["/docs/acls"],
                "root": ["/docs/root"],
            }
        }
    }))
}

async fn health() -> impl axum::response::IntoResponse {
    let now = chrono::Utc::now();

    match crate::database::manager::DatabaseManager::health_check().await {
        Ok(_) => (
            axum::http::StatusCode::OK,
            axum::response::Json(json!({
                "success": true,
                "data": {
                    "status": "ok",
                    "timestamp": now,
                    "database": "ok"
                }
            })),
        ),
        Err(e) => (
            axum::http::StatusCode::SERVICE_UNAVAILABLE,
            axum::response::Json(json!({
                "success": false,
                "error": "database unavailable",
                "data": {
                    "status": "degraded",
                    "timestamp": now,
                    "database_error": e.to_string()
                }
            })),
        ),
    }
}
