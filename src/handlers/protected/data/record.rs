use axum::{
    extract::{Path, Query},
    http::StatusCode,
    response::{IntoResponse, Json},
};
use serde::Deserialize;
use serde_json::{json, Value};
use sqlx::Row;

use crate::api::format::record_to_api_value;
use crate::database::manager::DatabaseManager;
use crate::observer::stateful_record::{RecordOperation, StatefulRecord};

use super::utils::{metadata_options_from_query, resolve_tenant_db};

#[derive(Debug, Deserialize)]
pub struct RecordQuery {
    /// Tenant database name (e.g., tenant_007314608dd04169). If omitted, falls back to MONK_TENANT_DB env.
    pub tenant: Option<String>,
    /// Include metadata sections. Examples: meta=true, meta=system,permissions
    pub meta: Option<String>,
}

/// GET /api/data/:schema/:id - Get a single record by ID
pub async fn get(
    Path((schema, id)): Path<(String, String)>,
    Query(query): Query<RecordQuery>,
) -> impl IntoResponse {
    // Resolve tenant database
    let tenant_db = match resolve_tenant_db(&query.tenant) {
        Ok(db) => db,
        Err(msg) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(json!({"success": false, "error": msg })),
            )
                .into_response()
        }
    };

    // Query the record using row_to_json for automatic column mapping
    let sql = format!(
        "SELECT row_to_json(t) AS row FROM (SELECT * FROM \"{}\" WHERE id = $1 AND \"trashed_at\" IS NULL AND \"deleted_at\" IS NULL) t",
        schema
    );

    let pool = match DatabaseManager::tenant_pool(&tenant_db).await {
        Ok(p) => p,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"success": false, "error": format!("database error: {}", e)})),
            )
                .into_response()
        }
    };

    let row = match sqlx::query(&sql).bind(&id).fetch_optional(&pool).await {
        Ok(Some(r)) => r,
        Ok(None) => {
            return (
                StatusCode::NOT_FOUND,
                Json(json!({"success": false, "error": format!("record {} not found in {}", id, schema)})),
            )
                .into_response()
        }
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"success": false, "error": format!("query failed: {}", e)})),
            )
                .into_response()
        }
    };

    let v: Value = row.try_get("row").unwrap_or(Value::Null);
    if let Value::Object(map) = v {
        let mut rec = StatefulRecord::existing(map.clone(), None, RecordOperation::NoChange);
        rec.extract_system_metadata();

        let options = metadata_options_from_query(query.meta.as_deref());
        let data = record_to_api_value(&rec, &schema, &options);

        Json(json!({ "success": true, "data": data })).into_response()
    } else {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"success": false, "error": "unexpected record format"})),
        )
            .into_response()
    }
}

/// PUT /api/data/:schema/:id - Update a record by ID
pub async fn put(
    Path((schema, id)): Path<(String, String)>,
    Query(query): Query<RecordQuery>,
    Json(payload): Json<Value>,
) -> impl IntoResponse {
    let _tenant_db = match resolve_tenant_db(&query.tenant) {
        Ok(db) => db,
        Err(msg) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(json!({"success": false, "error": msg })),
            )
        }
    };

    // TODO: Implement record update
    // 1. Fetch existing record
    // 2. Create StatefulRecord with RecordOperation::Update
    // 3. Apply changes through observer pipeline
    // 4. Update in database
    // 5. Return updated record

    (
        StatusCode::NOT_IMPLEMENTED,
        Json(json!({
            "success": false,
            "error": format!("PUT /api/data/{}/{} not yet implemented", schema, id),
            "message": "This will update a specific record"
        })),
    )
}

/// PATCH /api/data/:schema/:id - Partially update a record by ID
pub async fn patch(
    Path((schema, id)): Path<(String, String)>,
    Query(query): Query<RecordQuery>,
    Json(payload): Json<Value>,
) -> impl IntoResponse {
    let _tenant_db = match resolve_tenant_db(&query.tenant) {
        Ok(db) => db,
        Err(msg) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(json!({"success": false, "error": msg })),
            )
        }
    };

    // TODO: Implement partial record update
    // 1. Fetch existing record
    // 2. Merge changes
    // 3. Create StatefulRecord with RecordOperation::Update
    // 4. Apply through observer pipeline
    // 5. Update in database
    // 6. Return updated record

    (
        StatusCode::NOT_IMPLEMENTED,
        Json(json!({
            "success": false,
            "error": format!("PATCH /api/data/{}/{} not yet implemented", schema, id),
            "message": "This will partially update a specific record"
        })),
    )
}

/// DELETE /api/data/:schema/:id - Delete a record by ID
pub async fn delete(
    Path((schema, id)): Path<(String, String)>,
    Query(query): Query<RecordQuery>,
) -> impl IntoResponse {
    let _tenant_db = match resolve_tenant_db(&query.tenant) {
        Ok(db) => db,
        Err(msg) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(json!({"success": false, "error": msg })),
            )
        }
    };

    // TODO: Implement record deletion (soft delete by default)
    // 1. Fetch existing record
    // 2. Create StatefulRecord with RecordOperation::Delete
    // 3. Set deleted_at timestamp through observer pipeline
    // 4. Update in database
    // 5. Return success status

    (
        StatusCode::NOT_IMPLEMENTED,
        Json(json!({
            "success": false,
            "error": format!("DELETE /api/data/{}/{} not yet implemented", schema, id),
            "message": "This will soft delete a specific record"
        })),
    )
}

/// POST /api/data/:schema/:id/restore - Restore a soft-deleted record
pub async fn restore(
    Path((schema, id)): Path<(String, String)>,
    Query(query): Query<RecordQuery>,
) -> impl IntoResponse {
    let _tenant_db = match resolve_tenant_db(&query.tenant) {
        Ok(db) => db,
        Err(msg) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(json!({"success": false, "error": msg })),
            )
        }
    };

    // TODO: Implement record restoration
    // 1. Fetch soft-deleted record
    // 2. Clear deleted_at timestamp
    // 3. Update through observer pipeline
    // 4. Return restored record

    (
        StatusCode::NOT_IMPLEMENTED,
        Json(json!({
            "success": false,
            "error": format!("POST /api/data/{}/{}/restore not yet implemented", schema, id),
            "message": "This will restore a soft-deleted record"
        })),
    )
}