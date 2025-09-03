// handlers/elevated/root/tenant/update.rs - PATCH /api/root/tenant/:name handler

use axum::{extract::Path, http::StatusCode, response::Json};
use serde_json::{json, Value};

pub async fn tenant_update(Path(name): Path<String>) -> (StatusCode, Json<Value>) {
    (StatusCode::NOT_IMPLEMENTED, Json(json!({
        "error": "Tenant update not yet implemented",
        "message": format!("This will update configuration for tenant '{}'", name)
    })))
}