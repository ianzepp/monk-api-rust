// handlers/elevated/root/tenant/health.rs - GET /api/root/tenant/:name/health handler

use axum::{extract::Path, http::StatusCode, response::Json};
use serde_json::{json, Value};

pub async fn tenant_health(Path(name): Path<String>) -> (StatusCode, Json<Value>) {
    (StatusCode::NOT_IMPLEMENTED, Json(json!({
        "error": "Tenant health check not yet implemented",
        "message": format!("This will check database health for tenant '{}'", name)
    })))
}