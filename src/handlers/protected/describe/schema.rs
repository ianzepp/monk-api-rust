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
pub struct DescribeQuery {
    /// Include additional metadata. Examples: meta=true, meta=system,permissions
    pub meta: Option<String>,
}

/// GET /api/describe/:schema - Get JSON Schema definition for a schema
/// 
/// Returns the YAML JSON Schema definition that was used to create the PostgreSQL table.
/// This allows monk-cli to retrieve schema definitions for validation and tooling.
/// 
/// @param schema - The schema name (like "users", "products")  
/// @returns YAML JSON Schema definition or 404 if schema doesn't exist
pub async fn get(
    Path(schema): Path<String>,
    Query(query): Query<DescribeQuery>,
    Extension(TenantPool(pool)): Extension<TenantPool>,
    Extension(auth_user): Extension<AuthUser>,
) -> ApiResult<Value> {
    // Create DescribeService and get schema
    let service = DescribeService::new(pool);
    let schema_record = service.select_404(&schema).await
        .map_err(|e| match e {
            crate::services::describe_service::DescribeError::NotFound(_) => 
                ApiError::not_found(format!("Schema '{}' not found", schema)),
            _ => ApiError::internal_server_error("Failed to retrieve schema")
        })?;

    // Return the definition field from the schema record
    let definition = schema_record.get("definition")
        .ok_or_else(|| ApiError::internal_server_error("Schema definition missing"))?;
    
    Ok(ApiResponse::success(definition.clone()))
}

/// POST /api/describe/:schema - Create a new schema from JSON Schema definition
/// 
/// Accepts a JSON Schema definition and:
/// 1. Validates schema against JSON Schema specification
/// 2. Generates PostgreSQL CREATE TABLE statement
/// 3. Creates database table automatically
/// 4. Enables /api/data/:schema operations on new table
pub async fn post(
    Path(schema): Path<String>,
    Query(_query): Query<DescribeQuery>,
    Json(_payload): Json<Value>,
    Extension(TenantPool(pool)): Extension<TenantPool>,
    Extension(auth_user): Extension<AuthUser>,
) -> impl IntoResponse {
    // TODO: Implement schema creation
    // 1. Parse and validate JSON Schema definition
    // 2. Generate PostgreSQL DDL
    // 3. Execute DDL to create table
    // 4. Store schema definition in registry
    // 5. Return success with created schema info
    
    (
        StatusCode::NOT_IMPLEMENTED,
        Json(json!({
            "success": false,
            "error": format!("POST /api/describe/{} not yet implemented", schema),
            "message": "This will create a new schema from JSON Schema definition"
        })),
    )
}

/// PUT /api/describe/:schema - Update an existing schema definition
/// 
/// Accepts a JSON Schema definition and:
/// 1. Validates new schema definition
/// 2. Compares with existing schema for compatibility
/// 3. Generates ALTER TABLE statements for safe migrations
/// 4. Updates database table structure
/// 5. Updates schema registry
pub async fn put(
    Path(schema): Path<String>,
    Query(_query): Query<DescribeQuery>,
    Json(_payload): Json<Value>,
    Extension(TenantPool(pool)): Extension<TenantPool>,
    Extension(auth_user): Extension<AuthUser>,
) -> impl IntoResponse {
    // TODO: Implement schema update
    // 1. Fetch existing schema definition
    // 2. Validate new schema definition
    // 3. Generate migration SQL (ALTER TABLE statements)
    // 4. Execute migration
    // 5. Update schema registry
    // 6. Return success with migration summary
    
    (
        StatusCode::NOT_IMPLEMENTED,
        Json(json!({
            "success": false,
            "error": format!("PUT /api/describe/{} not yet implemented", schema),
            "message": "This will update an existing schema definition"
        })),
    )
}

/// DELETE /api/describe/:schema - Delete a schema and its associated table
/// 
/// WARNING: This is destructive and will:
/// 1. Drop the PostgreSQL table (losing all data)
/// 2. Remove schema definition from registry
/// 3. Disable all /api/data/:schema operations
pub async fn delete(
    Path(schema): Path<String>,
    Query(_query): Query<DescribeQuery>,
    Extension(TenantPool(pool)): Extension<TenantPool>,
    Extension(auth_user): Extension<AuthUser>,
) -> ApiResult<Value> {
    // Create DescribeService and delete schema
    let service = DescribeService::new(pool);
    let success = service.delete_one(&schema).await
        .map_err(|e| match e {
            crate::services::describe_service::DescribeError::NotFound(_) => 
                ApiError::not_found(format!("Schema '{}' not found", schema)),
            crate::services::describe_service::DescribeError::ProtectedSchema(name) => 
                ApiError::bad_request(format!("Schema '{}' is protected and cannot be deleted", name)),
            _ => ApiError::internal_server_error("Failed to delete schema")
        })?;

    if success {
        Ok(ApiResponse::success(json!({
            "deleted": true,
            "schema": schema,
            "message": "Schema marked for deletion"
        })))
    } else {
        Err(ApiError::not_found(format!("Schema '{}' not found", schema)))
    }
}

/*
SCHEMA MANAGEMENT IN RUST:

This endpoint is crucial for the monk ecosystem because:

1. **monk-cli Compatibility**: 
   - `monk meta select schema users` needs to get the YAML definition
   - Used for validation, documentation, and tooling
   
2. **Schema Evolution**:
   - Shows current schema version
   - Enables schema migration planning
   - Maintains history of schema changes
   
3. **Integration**:
   - Other services can introspect available schemas
   - API documentation can be auto-generated
   - Client-side validation uses these definitions

Implementation Plan:
```rust
// In production, this would:
pub async fn get(
    Path(schema): Path<String>,
    Extension(db): Extension<DatabasePool>,
) -> Result<Json<Value>, AppError> {
    // Query schema registry table  
    let schema_def = sqlx::query_as!(
        SchemaDefinition,
        "SELECT name, definition, created_at FROM schema_registry WHERE name = $1",
        schema
    )
    .fetch_optional(&db)
    .await?;
    
    match schema_def {
        Some(def) => Ok(Json(def.definition)),
        None => Err(AppError::NotFound(format!("Schema '{}' not found", schema)))
    }
}
```

This provides the same functionality as your TypeScript version but with:
- Compile-time SQL query validation
- Type-safe database operations  
- Zero-cost JSON serialization
- Memory safety guarantees

Key Files We'll Need:
- schema_validator.rs: JSON Schema validation logic
- ddl_generator.rs: PostgreSQL table generation  
- schema_registry.rs: In-memory schema caching
*/