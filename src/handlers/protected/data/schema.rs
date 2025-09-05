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

/// PUT /api/data/:schema - Upsert records (update if ID exists, create if no ID)
pub async fn put(
    Path(schema): Path<String>,
    Query(query): Query<ListQuery>,
    Json(payload): Json<Value>,
    Extension(TenantPool(pool)): Extension<TenantPool>,
    Extension(auth_user): Extension<AuthUser>,
) -> ApiResult<Value> {
    // Parse JSON array payload into Records
    let records = Record::from_json_array(payload)?;

    // Use Repository upsert_all method (handles splitting and operations internally)
    let repository = Repository::new(&schema, pool);
    let upserted_records = repository.upsert_all(records).await?;

    // Return array of all upserted records
    let data = Record::to_api_output_array(upserted_records);
    Ok(ApiResponse::success(data))
}

/// DELETE /api/data/:schema - Delete records by IDs from record array
pub async fn delete(
    Path(schema): Path<String>,
    Query(query): Query<ListQuery>,
    Json(payload): Json<Value>,
    Extension(TenantPool(pool)): Extension<TenantPool>,
    Extension(auth_user): Extension<AuthUser>,
) -> ApiResult<Value> {
    // Parse JSON array payload into Records
    let records = Record::from_json_array(payload)?;

    // Delete records directly (handles soft delete and ID validation via repository/observer pipeline)
    let repository = Repository::new(&schema, pool);
    let deleted_records = repository.delete_all(records).await?;

    // Return array of deleted records (with soft delete timestamps)
    let data = Record::to_api_output_array(deleted_records);
    Ok(ApiResponse::success(data))
}

/// PATCH /api/data/:schema - Update existing records (all records must have IDs)
pub async fn patch(
    Path(schema): Path<String>,
    Query(query): Query<ListQuery>,
    Json(payload): Json<Value>,
    Extension(TenantPool(pool)): Extension<TenantPool>,
    Extension(auth_user): Extension<AuthUser>,
) -> ApiResult<Value> {
    // Parse JSON array payload into Records
    let records = Record::from_json_array(payload)?;

    // Update all records (ID validation and 404 handling via repository/observer pipeline)
    let repository = Repository::new(&schema, pool);
    let updated_records = repository.update_all(records).await?;

    // Return array of updated records
    let data = Record::to_api_output_array(updated_records);
    Ok(ApiResponse::success(data))
}
