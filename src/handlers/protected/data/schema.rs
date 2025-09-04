use axum::{
    extract::{Path, Query},
    http::StatusCode,
    response::{IntoResponse, Json},
};
use serde::Deserialize;
use serde_json::{json, Value};

use crate::api::format::record_to_api_value;
use crate::database::dynamic::DynamicRepository;
use crate::database::manager::DatabaseManager;
use crate::filter::FilterData;

use crate::observer::stateful_record::{RecordOperation, StatefulRecord};

use super::utils::{metadata_options_from_query, resolve_tenant_db};

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

/// GET /api/data/:schema - List all records in a schema
pub async fn get(Path(schema): Path<String>, Query(query): Query<ListQuery>) -> impl IntoResponse {
    // Resolve tenant database
    let tenant_db = match resolve_tenant_db(&query.tenant) {
        Ok(db) => db,
        Err(msg) => {
            return (StatusCode::BAD_REQUEST, Json(json!({"success": false, "error": msg })))
                .into_response()
        }
    };

    let pool = match DatabaseManager::tenant_pool(&tenant_db).await {
        Ok(p) => p,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"success": false, "error": format!("database error: {}", e) })),
            )
                .into_response()
        }
    };

    // Build filter data with standard conditions for non-deleted records
    let filter_data = FilterData {
        select: None, // Select all columns
        where_clause: Some(json!({
            "$and": [
                { "trashed_at": { "$is": null } },
                { "deleted_at": { "$is": null } }
            ]
        })),
        order: None,
        limit: query.limit.map(|l| l.max(0) as i32),
        offset: query.offset.map(|o| o.max(0) as i32),
    };

    // Use DynamicRepository instead of raw SQL
    let repository = DynamicRepository::new(&schema, pool);
    let record_maps = match repository.select_any(filter_data).await {
        Ok(maps) => maps,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"success": false, "error": format!("query failed: {}", e) })),
            )
                .into_response()
        }
    };

    // Convert maps to StatefulRecord instances
    let mut records = Vec::new();
    for map in record_maps {
        let mut rec = StatefulRecord::existing(map, None, RecordOperation::NoChange);
        rec.extract_system_metadata();
        records.push(rec);
    }

    let options = metadata_options_from_query(query.meta.as_deref());
    let data: Vec<Value> =
        records.iter().map(|r| record_to_api_value(r, &schema, &options)).collect();

    Json(json!({ "success": true, "data": data })).into_response()
}

/// POST /api/data/:schema - Create a new record in the schema
pub async fn post(
    Path(schema): Path<String>,
    Query(query): Query<ListQuery>,
    Json(payload): Json<Value>,
) -> impl IntoResponse {
    // Resolve tenant database
    let tenant_db = match resolve_tenant_db(&query.tenant) {
        Ok(db) => db,
        Err(msg) => {
            return (StatusCode::BAD_REQUEST, Json(json!({"success": false, "error": msg })))
        }
    };

    // TODO: Implement record creation through observer pipeline
    // 1. Validate input data against schema
    // 2. Create StatefulRecord with RecordOperation::Create
    // 3. Execute through observer pipeline
    // 4. Insert into database
    // 5. Return created record with metadata

    (
        StatusCode::NOT_IMPLEMENTED,
        Json(json!({
            "success": false,
            "error": format!("POST /api/data/{} not yet implemented", schema),
            "message": "This will create new records in the schema"
        })),
    )
}

/// PUT /api/data/:schema - Bulk update records in the schema
pub async fn put(
    Path(schema): Path<String>,
    Query(query): Query<ListQuery>,
    Json(payload): Json<Value>,
) -> impl IntoResponse {
    let _tenant_db = match resolve_tenant_db(&query.tenant) {
        Ok(db) => db,
        Err(msg) => {
            return (StatusCode::BAD_REQUEST, Json(json!({"success": false, "error": msg })))
        }
    };

    // TODO: Implement bulk update
    // 1. Parse filter criteria from payload
    // 2. Fetch matching records
    // 3. Apply updates through observer pipeline
    // 4. Execute bulk update query
    // 5. Return updated records

    (
        StatusCode::NOT_IMPLEMENTED,
        Json(json!({
            "success": false,
            "error": format!("PUT /api/data/{} not yet implemented", schema),
            "message": "This will perform bulk updates on records"
        })),
    )
}

/// DELETE /api/data/:schema - Bulk delete records in the schema
pub async fn delete(
    Path(schema): Path<String>,
    Query(query): Query<ListQuery>,
) -> impl IntoResponse {
    let _tenant_db = match resolve_tenant_db(&query.tenant) {
        Ok(db) => db,
        Err(msg) => {
            return (StatusCode::BAD_REQUEST, Json(json!({"success": false, "error": msg })))
        }
    };

    // TODO: Implement bulk delete (soft delete by default)
    // 1. Parse filter criteria from query params
    // 2. Fetch matching records
    // 3. Set deleted_at timestamp through observer pipeline
    // 4. Execute soft delete update
    // 5. Return count of deleted records

    (
        StatusCode::NOT_IMPLEMENTED,
        Json(json!({
            "success": false,
            "error": format!("DELETE /api/data/{} not yet implemented", schema),
            "message": "This will perform bulk soft deletes on records"
        })),
    )
}

/// PATCH /api/data/:schema - Partial bulk update of records
pub async fn patch(
    Path(schema): Path<String>,
    Query(query): Query<ListQuery>,
    Json(payload): Json<Value>,
) -> impl IntoResponse {
    let _tenant_db = match resolve_tenant_db(&query.tenant) {
        Ok(db) => db,
        Err(msg) => {
            return (StatusCode::BAD_REQUEST, Json(json!({"success": false, "error": msg })))
        }
    };

    // TODO: Implement partial bulk update
    // Similar to PUT but only updates specified fields

    (
        StatusCode::NOT_IMPLEMENTED,
        Json(json!({
            "success": false,
            "error": format!("PATCH /api/data/{} not yet implemented", schema),
            "message": "This will perform partial bulk updates on records"
        })),
    )
}
