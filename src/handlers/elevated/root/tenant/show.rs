// handlers/elevated/root/tenant/show.rs - GET /api/root/tenant/:name handler

use axum::{extract::Path, http::StatusCode, response::Json};
use serde_json::{json, Value};

pub async fn tenant_show(Path(name): Path<String>) -> (StatusCode, Json<Value>) {
    (StatusCode::NOT_IMPLEMENTED, Json(json!({
        "error": "Tenant details not yet implemented", 
        "message": format!("This will show detailed information for tenant '{}'", name)
    })))
}