use axum::{
    extract::{Extension, Path, Query},
    http::StatusCode,
    response::{IntoResponse, Json},
};
use serde::Deserialize;
use serde_json::{json, Value};
use uuid::Uuid;

use crate::database::repository::Repository;
use crate::database::record::Record;
use crate::error::ApiError;
use crate::middleware::{TenantPool, AuthUser, ApiResponse, ApiResult};

use super::utils::metadata_options_from_query;

#[derive(Debug, Deserialize)]
pub struct RecordQuery {
    /// Include metadata sections. Examples: meta=true, meta=system,permissions
    pub meta: Option<String>,
}

/// GET /api/data/:schema/:id - Get a single record by ID
pub async fn get(
    Path((schema, id)): Path<(String, String)>,
    Query(query): Query<RecordQuery>,
    Extension(TenantPool(pool)): Extension<TenantPool>,
    Extension(auth_user): Extension<AuthUser>,
) -> ApiResult<Value> {
    // Parse ID as UUID
    let record_id: Uuid = id.parse()
        .map_err(|_| ApiError::bad_request(format!("Invalid UUID format: {}", id)))?;

    // Use Repository to select single record by ID
    let repository = Repository::new(&schema, pool);
    let record = repository.select_404(record_id).await?;

    // Return single record (not array)
    let data = record.to_api_output();
    Ok(ApiResponse::success(data))
}

/// PUT /api/data/:schema/:id - Update a record by ID (upsert behavior)
pub async fn put(
    Path((schema, id)): Path<(String, String)>,
    Query(query): Query<RecordQuery>,
    Json(payload): Json<Value>,
    Extension(TenantPool(pool)): Extension<TenantPool>,
    Extension(auth_user): Extension<AuthUser>,
) -> ApiResult<Value> {
    // Parse ID as UUID
    let record_id: Uuid = id.parse()
        .map_err(|_| ApiError::bad_request(format!("Invalid UUID format: {}", id)))?;

    // Create Record from payload and set the ID
    let mut record = Record::from_json_object(payload)?;
    record.set_id(record_id);

    // Use Repository upsert (update if exists, create if not)
    let repository = Repository::new(&schema, pool);
    let upserted_record = repository.upsert_one(record).await?;

    // Return single updated/created record
    let data = upserted_record.to_api_output();
    Ok(ApiResponse::success(data))
}

/// PATCH /api/data/:schema/:id - Partially update a record by ID
pub async fn patch(
    Path((schema, id)): Path<(String, String)>,
    Query(query): Query<RecordQuery>,
    Json(payload): Json<Value>,
    Extension(TenantPool(pool)): Extension<TenantPool>,
    Extension(auth_user): Extension<AuthUser>,
) -> ApiResult<Value> {
    // Parse ID as UUID
    let record_id: Uuid = id.parse()
        .map_err(|_| ApiError::bad_request(format!("Invalid UUID format: {}", id)))?;

    // Create Record with partial updates
    let updates_record = Record::from_json_object(payload)?;

    // Use Repository update_404 (requires record to exist)
    let repository = Repository::new(&schema, pool);
    let updated_record = repository.update_404(record_id, updates_record).await?;

    // Return single updated record
    let data = updated_record.to_api_output();
    Ok(ApiResponse::success(data))
}

/// DELETE /api/data/:schema/:id - Delete a record by ID
pub async fn delete(
    Path((schema, id)): Path<(String, String)>,
    Query(query): Query<RecordQuery>,
    Extension(TenantPool(pool)): Extension<TenantPool>,
    Extension(auth_user): Extension<AuthUser>,
) -> ApiResult<Value> {
    // Parse ID as UUID
    let record_id: Uuid = id.parse()
        .map_err(|_| ApiError::bad_request(format!("Invalid UUID format: {}", id)))?;

    // Use Repository delete_404 (requires record to exist, handles soft delete)
    let repository = Repository::new(&schema, pool);
    let deleted_record = repository.delete_404(record_id).await?;

    // Return single deleted record (with soft delete timestamps)
    let data = deleted_record.to_api_output();
    Ok(ApiResponse::success(data))
}

/// POST /api/data/:schema/:id/restore - Restore a soft-deleted record
pub async fn restore(
    Path((schema, id)): Path<(String, String)>,
    Query(query): Query<RecordQuery>,
    Extension(TenantPool(pool)): Extension<TenantPool>,
    Extension(auth_user): Extension<AuthUser>,
) -> ApiResult<Value> {
    // Parse ID as UUID
    let record_id: Uuid = id.parse()
        .map_err(|_| ApiError::bad_request(format!("Invalid UUID format: {}", id)))?;

    // TODO: Implement record restoration in Repository
    // For now, return not implemented with clean API structure
    Err(ApiError::not_implemented(format!(
        "POST /api/data/{}/{}/restore not yet implemented", schema, id
    )))
}