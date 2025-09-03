// handlers/elevated/root/tenant/list.rs - GET /api/root/tenant handler

use axum::{http::StatusCode, response::Json};
use serde_json::{json, Value};

pub async fn tenant_list() -> (StatusCode, Json<Value>) {
    (StatusCode::NOT_IMPLEMENTED, Json(json!({
        "error": "Tenant listing not yet implemented",
        "message": "This will list all tenants with health status and metrics"
    })))
}