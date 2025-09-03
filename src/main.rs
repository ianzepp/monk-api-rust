use axum::{
    http::StatusCode,
    response::Json,
    routing::{get, post},
    Router,
};
use serde_json::{json, Value};
use tower_http::cors::CorsLayer;
use tower_http::trace::TraceLayer;

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();

    let app = Router::new()
        .route("/", get(root))
        .route("/health", get(health))
        .route("/api/auth/login", post(auth_login_placeholder))
        .route("/api/data/:schema", get(data_list_placeholder))
        .route("/api/meta/schema/:name", get(meta_schema_placeholder))
        .layer(CorsLayer::permissive())
        .layer(TraceLayer::new_for_http());

    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await.unwrap();
    println!("🚀 Monk API Rust server listening on http://0.0.0.0:3000");
    
    axum::serve(listener, app).await.unwrap();
}

async fn root() -> Json<Value> {
    Json(json!({
        "name": "monk-api-rust",
        "version": "0.1.0",
        "status": "development",
        "description": "Rust rewrite of Monk API - PaaS management platform"
    }))
}

async fn health() -> Json<Value> {
    Json(json!({
        "status": "ok",
        "timestamp": chrono::Utc::now()
    }))
}

// Placeholder handlers to be implemented
async fn auth_login_placeholder() -> (StatusCode, Json<Value>) {
    (StatusCode::NOT_IMPLEMENTED, Json(json!({
        "error": "Authentication endpoints not yet implemented",
        "message": "This is a development placeholder"
    })))
}

async fn data_list_placeholder() -> (StatusCode, Json<Value>) {
    (StatusCode::NOT_IMPLEMENTED, Json(json!({
        "error": "Data endpoints not yet implemented", 
        "message": "This is a development placeholder"
    })))
}

async fn meta_schema_placeholder() -> (StatusCode, Json<Value>) {
    (StatusCode::NOT_IMPLEMENTED, Json(json!({
        "error": "Meta endpoints not yet implemented",
        "message": "This is a development placeholder"
    })))
}