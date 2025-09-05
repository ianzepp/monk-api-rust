use axum::{
    extract::{Extension, Path, Query},
    http::StatusCode,
    response::{IntoResponse, Json},
};
use serde::Deserialize;
use serde_json::{json, Value};

use crate::database::repository::Repository;
use crate::database::record::{Record, RecordVecExt};
use crate::filter::FilterData;
use crate::error::ApiError;
use crate::middleware::{TenantPool, AuthUser, ApiResponse, ApiResult};

#[derive(Debug, Deserialize)]
pub struct FindQuery {
    /// Include metadata sections. Examples: meta=true, meta=system,permissions
    pub meta: Option<String>,
}

/// POST /api/find/:schema - Advanced filtered search
/// 
/// Accepts a FilterData JSON body with:
/// - select: fields to return
/// - where: filter conditions
/// - order: sort order
/// - limit/offset: pagination
pub async fn post(
    Path(schema): Path<String>,
    Query(query): Query<FindQuery>,
    Json(filter_data): Json<FilterData>,
    Extension(TenantPool(pool)): Extension<TenantPool>,
    Extension(auth_user): Extension<AuthUser>,
) -> ApiResult<Value> {
    // Use Repository to select records with filter criteria
    let repository = Repository::new(&schema, pool);
    let records = repository.select_any(filter_data).await?;

    // Return array of matching records
    let data = records.to_api();
    Ok(ApiResponse::success(data))
}

/// DELETE /api/find/:schema - Bulk delete matching records
/// 
/// Accepts FilterData for the search criteria
pub async fn delete(
    Path(schema): Path<String>,
    Query(query): Query<FindQuery>,
    Json(filter_data): Json<FilterData>,
    Extension(TenantPool(pool)): Extension<TenantPool>,
    Extension(auth_user): Extension<AuthUser>,
) -> ApiResult<Value> {
    // Use Repository to delete records matching filter criteria
    let repository = Repository::new(&schema, pool);
    let deleted_records = repository.delete_any(filter_data).await?;

    // Return array of deleted records (with soft delete timestamps)
    let data = deleted_records.to_api();
    Ok(ApiResponse::success(data))
}