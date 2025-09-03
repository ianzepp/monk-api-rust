// handlers/meta/schema_delete.rs - DELETE /api/meta/:schema handler
use axum::{extract::Path, http::StatusCode, response::Json};
use serde_json::{json, Value};

pub async fn schema_delete(Path(schema): Path<String>) -> (StatusCode, Json<Value>) {
    (StatusCode::NOT_IMPLEMENTED, Json(json!({
        "error": format!("DELETE /api/meta/{} not yet implemented", schema),
        "message": "This will soft delete a schema definition"
    })))
}