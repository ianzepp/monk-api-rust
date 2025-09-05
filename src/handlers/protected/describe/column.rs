use axum::{
    extract::{Extension, Path, Query},
    http::StatusCode,
    response::{IntoResponse, Json},
};
use serde::Deserialize;
use serde_json::{json, Value};

use crate::services::describe_service::DescribeService;
use crate::middleware::{TenantPool, AuthUser, ApiResponse, ApiResult};
use crate::error::ApiError;

#[derive(Debug, Deserialize)]
pub struct ColumnQuery {
    /// Include additional metadata. Examples: meta=true, meta=system,permissions
    pub meta: Option<String>,
}

/// GET /api/describe/:schema/:column - Get column definition
/// 
/// Returns the column metadata including type, constraints, validation rules, etc.
/// This provides detailed information about a specific column within a schema.
/// 
/// @param schema - The schema name (like "users", "products")  
/// @param column - The column name (like "email", "price")
/// @returns Column definition with validation metadata or 404 if not found
pub async fn get(
    Path((schema, column)): Path<(String, String)>,
    Query(query): Query<ColumnQuery>,
    Extension(TenantPool(pool)): Extension<TenantPool>,
    Extension(auth_user): Extension<AuthUser>,
) -> ApiResult<Value> {
    let service = DescribeService::new(pool);
    let column_record = service.select_column_404(&schema, &column).await?;

    // Return the column metadata as JSON
    let column_data = column_record.to_api_output();
    Ok(ApiResponse::success(column_data))
}

/// POST /api/describe/:schema/:column - Add new column to existing schema
/// 
/// Accepts a column definition (subset of JSON Schema property) and:
/// 1. Validates the column definition
/// 2. Generates ALTER TABLE statement to add column
/// 3. Updates database table structure
/// 4. Creates column record in metadata
pub async fn post(
    Path((schema, column)): Path<(String, String)>,
    Query(query): Query<ColumnQuery>,
    Json(payload): Json<Value>,
    Extension(TenantPool(pool)): Extension<TenantPool>,
    Extension(auth_user): Extension<AuthUser>,
) -> ApiResult<Value> {
    // Extract required flag from query params or default to false
    let is_required = query.meta
        .as_ref()
        .and_then(|m| if m.contains("required") { Some(true) } else { None })
        .unwrap_or(false);

    let service = DescribeService::new(pool);
    let created_column = service.create_column(&schema, &column, payload, is_required).await?;

    Ok(ApiResponse::success(json!({
        "created": true,
        "schema": schema,
        "column": column,
        "message": "Column added successfully"
    })))
}

/// PATCH /api/describe/:schema/:column - Update existing column definition
/// 
/// Accepts partial column definition updates and:
/// 1. Validates the new column properties
/// 2. Compares with existing column for compatibility
/// 3. Generates ALTER TABLE statements if needed
/// 4. Updates column metadata
pub async fn patch(
    Path((schema, column)): Path<(String, String)>,
    Query(query): Query<ColumnQuery>,
    Json(payload): Json<Value>,
    Extension(TenantPool(pool)): Extension<TenantPool>,
    Extension(auth_user): Extension<AuthUser>,
) -> ApiResult<Value> {
    // Extract required flag from query params (optional for updates)
    let is_required = query.meta
        .as_ref()
        .and_then(|m| if m.contains("required") { Some(true) } else if m.contains("optional") { Some(false) } else { None });

    let service = DescribeService::new(pool);
    let updated_column = service.update_column_404(&schema, &column, payload, is_required).await?;

    Ok(ApiResponse::success(json!({
        "updated": true,
        "schema": schema,
        "column": column,
        "message": "Column updated successfully"
    })))
}

/// DELETE /api/describe/:schema/:column - Drop column from schema
/// 
/// WARNING: This is destructive and will:
/// 1. Drop the column from PostgreSQL table (losing all data in that column)
/// 2. Remove column metadata from registry
/// 3. Update schema field_count
pub async fn delete(
    Path((schema, column)): Path<(String, String)>,
    Query(_query): Query<ColumnQuery>,
    Extension(TenantPool(pool)): Extension<TenantPool>,
    Extension(auth_user): Extension<AuthUser>,
) -> ApiResult<Value> {
    let service = DescribeService::new(pool);
    service.delete_column_404(&schema, &column).await?;

    Ok(ApiResponse::success(json!({
        "deleted": true,
        "schema": schema,
        "column": column,
        "message": "Column dropped successfully"
    })))
}

/*
COLUMN MANAGEMENT IN RUST:

This endpoint family provides fine-grained column management within schemas:

1. **Individual Column Operations**: 
   - Add/modify/remove single columns without full schema replacement
   - Safer for production environments with existing data
   - Enables incremental schema evolution
   
2. **Schema Evolution**:
   - Column-level changes are more atomic and safer
   - Better migration control and rollback capabilities
   - Maintains column history and metadata
   
3. **Integration with Schema API**:
   - `/api/describe/:schema` operations can delegate to column operations
   - Schema updates become orchestrated column changes
   - Single source of truth in columns table

Column operations will work directly with the columns table metadata,
making them faster and more direct than full schema operations.
*/