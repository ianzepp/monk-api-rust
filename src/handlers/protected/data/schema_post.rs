// handlers/data/schema_post.rs - POST /api/data/:schema handler
use axum::{extract::Path, http::StatusCode, response::Json};
use serde_json::{json, Value};

pub async fn schema_post(Path(schema): Path<String>) -> (StatusCode, Json<Value>) {
    (StatusCode::NOT_IMPLEMENTED, Json(json!({
        "error": format!("POST /api/data/{} not yet implemented", schema),
        "message": "This will create new records in the schema"
    })))
}