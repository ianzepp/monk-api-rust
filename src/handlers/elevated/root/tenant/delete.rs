// handlers/elevated/root/tenant/delete.rs - DELETE /api/root/tenant/:name handler

use axum::{extract::Path, http::StatusCode, response::Json};
use serde_json::{json, Value};

pub async fn tenant_delete(Path(name): Path<String>) -> (StatusCode, Json<Value>) {
    (StatusCode::NOT_IMPLEMENTED, Json(json!({
        "error": "Tenant deletion not yet implemented",
        "message": format!("This will soft delete tenant '{}' (preserves data)", name)
    })))
}