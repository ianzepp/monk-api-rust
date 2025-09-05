use axum::{
    extract::{Extension, Path, Query},
    http::StatusCode,
    response::{IntoResponse, Json},
};
use serde::Deserialize;
use serde_json::{json, Value};

use crate::database::repository::Repository;
use crate::database::record::Record;
use crate::filter::FilterData;
use crate::error::ApiError;
use crate::middleware::{TenantPool, AuthUser, ApiResponse, ApiResult};

use super::utils::metadata_options_from_query;

#[derive(Debug, Deserialize)]
pub struct ListQuery {
    /// Include metadata sections. Examples: meta=true, meta=system,permissions
    pub meta: Option<String>,
    /// Pagination (optional)
    pub limit: Option<i64>,
    pub offset: Option<i64>,
}

/// GET /api/data/:schema - List all records in a schema
pub async fn get(
    Path(schema): Path<String>, 
    Query(query): Query<ListQuery>,
    Extension(TenantPool(pool)): Extension<TenantPool>,
    Extension(auth_user): Extension<AuthUser>,
) -> ApiResult<Value> {
    // Use Repository with clean select_all method
    let repository = Repository::new(&schema, pool);
    let records = repository.select_all(
        query.limit.map(|l| l.max(0) as i32),
        query.offset.map(|o| o.max(0) as i32)
    ).await?;

    // Use Record's ergonomic API output helper and return clean data
    let data = Record::to_api_output_array(records);
    Ok(ApiResponse::success(data))
}

/// POST /api/data/:schema - Create multiple records in the schema (bulk operation)
pub async fn post(
    Path(schema): Path<String>,
    Query(query): Query<ListQuery>,
    Json(payload): Json<Value>,
    Extension(TenantPool(pool)): Extension<TenantPool>,
    Extension(auth_user): Extension<AuthUser>,
) -> ApiResult<Value> {
    // Parse JSON array payload into Records
    let records = Record::from_json_array(payload)?;

    // Use Repository to create all records (handles observer pipeline)
    let repository = Repository::new(&schema, pool);
    let created_records = repository.create_all(records).await?;

    // Return array of created records with 201 Created status
    let data = Record::to_api_output_array(created_records);
    Ok(ApiResponse::created(data))
}

/// PUT /api/data/:schema - Bulk update records with filter criteria
pub async fn put(
    Path(schema): Path<String>,
    Query(query): Query<ListQuery>,
    Json(payload): Json<Value>,
    Extension(TenantPool(pool)): Extension<TenantPool>,
    Extension(auth_user): Extension<AuthUser>,
) -> ApiResult<Value> {
    // Expect payload with filter and updates
    // { "filter": { "status": "draft" }, "updates": { "status": "published", "published_at": "2024-01-15" } }
    let filter_data = payload.get("filter")
        .and_then(|f| serde_json::from_value::<FilterData>(f.clone()).ok())
        .unwrap_or_default();
    
    let updates = payload.get("updates")
        .ok_or_else(|| ApiError::bad_request("Missing 'updates' field in payload"))?
        .clone();

    // Fetch matching records
    let repository = Repository::new(&schema, pool);
    let mut records = repository.select_any(filter_data).await?;

    // Apply updates to each record
    let updates_map = Record::json_to_hashmap(updates)?;
    for record in &mut records {
        record.apply_changes(updates_map.clone());
    }

    // Bulk update all records (handles observer pipeline)
    let updated_records = repository.update_all(records).await?;

    // Return array of updated records
    let data = Record::to_api_output_array(updated_records);
    Ok(ApiResponse::success(data))
}

/// DELETE /api/data/:schema - Bulk delete records with filter criteria
pub async fn delete(
    Path(schema): Path<String>,
    Query(query): Query<ListQuery>,
    Extension(TenantPool(pool)): Extension<TenantPool>,
    Extension(auth_user): Extension<AuthUser>,
) -> ApiResult<Value> {
    // For DELETE, filter criteria can come from query params or request body
    // For now, support basic query params (limit/offset) and match all
    let filter_data = FilterData {
        select: None,
        where_clause: None, // Could be extended to accept filter query params
        order: None,
        limit: query.limit.map(|l| l.max(0) as i32),
        offset: query.offset.map(|o| o.max(0) as i32),
    };

    // Fetch records to delete
    let repository = Repository::new(&schema, pool);
    let records = repository.select_any(filter_data).await?;

    if records.is_empty() {
        // No records found to delete
        return Ok(ApiResponse::success(serde_json::json!([])));
    }

    // Bulk delete records (handles soft delete via observer pipeline)
    let deleted_records = repository.delete_all(records).await?;

    // Return array of deleted records (with soft delete timestamps)
    let data = Record::to_api_output_array(deleted_records);
    Ok(ApiResponse::success(data))
}

/// PATCH /api/data/:schema - Partial bulk update of records (same as PUT)
pub async fn patch(
    Path(schema): Path<String>,
    Query(query): Query<ListQuery>,
    Json(payload): Json<Value>,
    Extension(TenantPool(pool)): Extension<TenantPool>,
    Extension(auth_user): Extension<AuthUser>,
) -> ApiResult<Value> {
    // PATCH is identical to PUT for schema-level operations
    // Both are partial updates with filter criteria
    put(Path(schema), Query(query), Json(payload), Extension(pool), Extension(auth_user)).await
}
