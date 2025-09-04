use axum::{extract::{Path, Query}, response::IntoResponse, Json};
use serde::Deserialize;
use serde_json::{json, Value, Map};
use sqlx::Row;
use uuid::Uuid;

use crate::api::format::{record_to_api_value, MetadataOptions};
use crate::database::manager::DatabaseManager;
use crate::observer::stateful_record::{RecordOperation, StatefulRecord};

#[derive(Debug, Deserialize)]
pub struct ListQuery {
    /// Tenant database name (e.g., tenant_007314608dd04169). If omitted, falls back to MONK_TENANT_DB env.
    pub tenant: Option<String>,
    /// Include metadata sections. Examples: meta=true, meta=system,permissions
    pub meta: Option<String>,
    /// Pagination (optional)
    pub limit: Option<i64>,
    pub offset: Option<i64>,
}

/// GET /api/data/:schema - list records
pub async fn schema_get(Path(schema): Path<String>, Query(query): Query<ListQuery>) -> impl IntoResponse {
    // Resolve tenant database
    let tenant_db = match resolve_tenant_db(&query.tenant) {
        Ok(db) => db,
        Err(msg) => return (axum::http::StatusCode::BAD_REQUEST, Json(json!({"success": false, "error": msg }))).into_response(),
    };

    // Build SQL query using row_to_json to avoid hand-mapping columns
    let mut inner = format!("SELECT * FROM \"{}\" WHERE \"trashed_at\" IS NULL AND \"deleted_at\" IS NULL", schema);
    if let Some(limit) = query.limit { inner.push_str(&format!(" LIMIT {}", limit.max(0))); }
    if let Some(offset) = query.offset { inner.push_str(&format!(" OFFSET {}", offset.max(0))); }
    let sql = format!("SELECT row_to_json(t) AS row FROM ({}) t", inner);

    let pool = match DatabaseManager::tenant_pool(&tenant_db).await {
        Ok(p) => p,
        Err(e) => return (axum::http::StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"success": false, "error": format!("database error: {}", e) })) ).into_response(),
    };

    let rows = match sqlx::query(&sql).fetch_all(&pool).await {
        Ok(r) => r,
        Err(e) => return (axum::http::StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"success": false, "error": format!("query failed: {}", e) })) ).into_response(),
    };

    let mut records = Vec::new();
    for row in rows {
        let v: Value = row.try_get("row").unwrap_or(Value::Null);
        if let Value::Object(map) = v {
            let mut rec = StatefulRecord::existing(map.clone(), None, RecordOperation::NoChange);
            rec.extract_system_metadata();
            records.push(rec);
        }
    }

    let options = metadata_options_from_query(query.meta.as_deref());
    let data: Vec<Value> = records.iter().map(|r| record_to_api_value(r, &schema, &options)).collect();

    Json(json!({ "success": true, "data": data }))
}

#[derive(Debug, Deserialize)]
pub struct ShowQuery {
    pub tenant: Option<String>,
    pub meta: Option<String>,
}

/// GET /api/data/:schema/:id - show single record by id
pub async fn schema_show(Path((schema, id)): Path<(String, String)>, Query(query): Query<ShowQuery>) -> impl IntoResponse {
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

fn resolve_tenant_db(param: &Option<String>) -> Result<String, String> {
    if let Some(db) = param { return Ok(db.clone()); }
    if let Ok(env_db) = std::env::var("MONK_TENANT_DB") { return Ok(env_db); }
    Err("tenant database not specified; provide ?tenant=tenant_<hash> or set MONK_TENANT_DB".to_string())
}

fn metadata_options_from_query(meta_param: Option<&str>) -> MetadataOptions {
    match meta_param {
        None => MetadataOptions::none(),
        Some("true") => MetadataOptions::all(),
        Some("false") | Some("") => MetadataOptions::none(),
        Some(param_value) => {
            // Parse comma-separated sections or dotted fields
            let mut opts = MetadataOptions::default();
            let mut specific = Vec::new();
            for part in param_value.split(',').map(|s| s.trim()).filter(|s| !s.is_empty()) {
                match part {
                    "system" => opts.include_system = true,
                    "computed" => opts.include_computed = true,
                    "permissions" => opts.include_permissions = true,
                    "relationships" => opts.include_relationships = true,
                    "processing" => opts.include_processing = true,
                    other if other.contains('.') => specific.push(other.to_string()),
                    _ => {}
                }
            }
            if !specific.is_empty() { opts.specific_fields = Some(specific); }
            opts
        }
    }
}
