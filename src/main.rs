use axum::{
    routing::get,
    Router,
};
use serde_json::{json, Value};
use tower_http::{cors::CorsLayer, trace::TraceLayer};

mod config;
mod database;
mod observer;
mod api;
mod handlers;
mod filter;

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
        // Protected API (auth skipped for MVP)
        .merge(data_routes())
        // Global middleware
        .layer(CorsLayer::permissive())
        .layer(TraceLayer::new_for_http())
}

fn data_routes() -> Router {
    use axum::routing::{delete, patch, post, put};
    use handlers::protected::{data, find};

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
        .route(
            "/api/data/:schema/:id/restore",
            post(data::record_restore),
        )
        // Find endpoint (search/filter)
        .route(
            "/api/find/:schema",
            post(find::find_post::find_post),
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
                "public_auth": "/auth/* (public - token acquisition)",
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
