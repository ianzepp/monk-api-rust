// handlers/elevated/root/tenant/restore.rs - PUT /api/root/tenant/:name handler

use axum::{extract::Path, http::StatusCode, response::Json};
use serde_json::{json, Value};

pub async fn tenant_restore(Path(name): Path<String>) -> (StatusCode, Json<Value>) {
    (StatusCode::NOT_IMPLEMENTED, Json(json!({
        "error": "Tenant restoration not yet implemented",
        "message": format!("This will restore soft-deleted tenant '{}'", name)
    })))
}