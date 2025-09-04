use axum::{extract::{Path, Query}, response::IntoResponse, Json};
use serde::Deserialize;
use serde_json::{json, Value};
use sqlx::Row;
use uuid::Uuid;

use crate::api::format::record_to_api_value;
use crate::database::manager::DatabaseManager;
use crate::observer::stateful_record::{RecordOperation, StatefulRecord};

use super::{metadata_options_from_query, resolve_tenant_db};

#[derive(Debug, Deserialize)]
pub struct ShowQuery {
    pub tenant: Option<String>,
    pub meta: Option<String>,
}

/// GET /api/data/:schema/:id - show single record by id
pub async fn record_get(Path((schema, id)): Path<(String, String)>, Query(query): Query<ShowQuery>) -> impl IntoResponse {
    let tenant_db = match resolve_tenant_db(&query.tenant) {
        Ok(db) => db,
        Err(msg) => return (axum::http::StatusCode::BAD_REQUEST, Json(json!({"success": false, "error": msg }))).into_response(),
    };

    let id_uuid = match Uuid::parse_str(&id) {
        Ok(u) => u,
        Err(_) => return (axum::http::StatusCode::BAD_REQUEST, Json(json!({"success": false, "error": "invalid UUID" }))).into_response(),
    };

    let sql = format!(
        "SELECT row_to_json(t) AS row FROM (SELECT * FROM \"{}\" WHERE id = $1 AND \"trashed_at\" IS NULL AND \"deleted_at\" IS NULL) t",
        schema
    );

    let pool = match DatabaseManager::tenant_pool(&tenant_db).await {
        Ok(p) => p,
        Err(e) => return (axum::http::StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"success": false, "error": format!("database error: {}", e) })) ).into_response(),
    };

    let row_opt = match sqlx::query(&sql).bind(id_uuid).fetch_optional(&pool).await {
        Ok(r) => r,
        Err(e) => return (axum::http::StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"success": false, "error": format!("query failed: {}", e) })) ).into_response(),
    };

    if let Some(row) = row_opt {
        let v: Value = row.try_get("row").unwrap_or(Value::Null);
        if let Value::Object(map) = v {
            let mut rec = StatefulRecord::existing(map.clone(), None, RecordOperation::NoChange);
            rec.extract_system_metadata();
            let options = metadata_options_from_query(query.meta.as_deref());
            let data = record_to_api_value(&rec, &schema, &options);
            return Json(json!({ "success": true, "data": data })).into_response();
        }
    }

    (axum::http::StatusCode::NOT_FOUND, Json(json!({ "success": false, "error": "not found" }))).into_response()
}
