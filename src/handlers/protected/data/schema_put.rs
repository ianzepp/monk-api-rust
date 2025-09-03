// handlers/data/schema_put.rs - PUT /api/data/:schema handler  
use axum::{extract::Path, http::StatusCode, response::Json};
use serde_json::{json, Value};

pub async fn schema_put(Path(schema): Path<String>) -> (StatusCode, Json<Value>) {
    (StatusCode::NOT_IMPLEMENTED, Json(json!({
        "error": format!("PUT /api/data/{} not yet implemented", schema),
        "message": "This will bulk update records in the schema"
    })))
}