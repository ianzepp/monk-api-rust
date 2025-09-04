# Transactions and API Responses - Rust Implementation

This document outlines how to implement the TypeScript transaction patterns and response formats in Rust, maintaining 100% compatibility with monk-cli while providing superior safety and performance.

## TypeScript Pattern Analysis

### Current TypeScript Architecture

```typescript
// Read-only operations
export default withParams(async (context, { system, schema, body, options }) => {
    const results = await system.database.selectAny(schema, body, options);
    setRouteResult(context, { success: true, data: results });
});

// Write operations with transactions
export default withTransactionParams(async (context, { system, schema, body }) => {
    // Multiple database operations in a single transaction
    const createdRecords = await system.database.createAll(schema, body);
    const relatedRecords = await system.database.selectAny('related', filter);
    
    setRouteResult(context, { success: true, data: createdRecords });
    // Auto-commit on success, auto-rollback on error
});
```

**Key TypeScript Features:**
1. **Transaction Boundaries**: Automatic BEGIN/COMMIT/ROLLBACK at API boundary
2. **Single Code Path**: Only `withTransactionParams()` manages transactions
3. **Consistent Responses**: Standardized success/error JSON format
4. **Error Propagation**: Automatic rollback on any thrown error

## Superior Rust Design

### Architecture Overview

```rust
// Axum handlers with transaction middleware
async fn create_records(
    Path(schema): Path<String>,
    Extension(ctx): Extension<RequestContext>, // Contains pool/transaction
    Json(data): Json<Vec<serde_json::Value>>,
) -> Result<ApiResponse<Vec<Record>>, ApiError> {
    
    // Use repository with transaction context
    let repo = DynamicRepository::new(&schema, &ctx.db);
    
    // All operations automatically use the transaction if present
    let created = repo.create_all(data).await?;
    let related = repo.select_any(related_filter).await?;
    
    // Success response - transaction auto-commits
    Ok(ApiResponse::success(created))
    // Error propagation - transaction auto-rollbacks
}
```

### 1. Request Context and Transaction Management

```rust
// src/api/context.rs
use sqlx::{PgPool, Transaction, Postgres};
use std::sync::Arc;

/// Request context that may contain a transaction
#[derive(Clone)]
pub struct RequestContext {
    pub pool: PgPool,
    pub transaction: Option<Arc<Transaction<'static, Postgres>>>,
    pub tenant_db: String,
    pub user_id: Option<Uuid>,
}

impl RequestContext {
    /// Get database context - transaction if available, otherwise pool
    pub fn db(&self) -> DatabaseContext<'_> {
        match &self.transaction {
            Some(tx) => DatabaseContext::Transaction(tx),
            None => DatabaseContext::Pool(&self.pool),
        }
    }
}

/// Unified database context for queries
pub enum DatabaseContext<'a> {
    Pool(&'a PgPool),
    Transaction(&'a Transaction<'a, Postgres>),
}

impl<'a> DatabaseContext<'a> {
    /// Execute query using either pool or transaction
    pub async fn execute<'q, A>(&self, query: sqlx::query::Query<'q, Postgres, A>) -> Result<PgQueryResult, sqlx::Error>
    where
        A: 'q + sqlx::IntoArguments<'q, Postgres>,
    {
        match self {
            Self::Pool(pool) => query.execute(*pool).await,
            Self::Transaction(tx) => query.execute(*tx).await,
        }
    }
    
    pub async fn fetch_all<'q, T>(&self, query: sqlx::query::QueryAs<'q, Postgres, T, <Postgres as sqlx::Database>::Arguments<'q>>) -> Result<Vec<T>, sqlx::Error>
    where
        T: for<'r> sqlx::FromRow<'r, <Postgres as sqlx::Database>::Row>,
    {
        match self {
            Self::Pool(pool) => query.fetch_all(*pool).await,
            Self::Transaction(tx) => query.fetch_all(*tx).await,
        }
    }
    
    // Add other query methods as needed...
}
```

### 2. Transaction Middleware

```rust
// src/api/middleware/transaction.rs
use axum::{extract::Request, middleware::Next, response::Response, Extension};
use sqlx::PgPool;
use tracing::{info, warn, error};

/// Middleware that wraps handlers with transaction management
/// Equivalent to TypeScript's withTransactionParams()
pub async fn with_transaction(
    Extension(pool): Extension<PgPool>,
    Extension(tenant_db): Extension<String>,
    mut request: Request,
    next: Next,
) -> Result<Response, ApiError> {
    // Begin transaction
    let mut tx = pool.begin().await.map_err(|e| {
        error!("Failed to begin transaction: {}", e);
        ApiError::database_error("Failed to start transaction")
    })?;
    
    info!("Transaction started for {} {}", 
          request.method(), 
          request.uri().path());
    
    // Create request context with transaction
    let ctx = RequestContext {
        pool: pool.clone(),
        transaction: Some(Arc::new(unsafe { 
            std::mem::transmute::<Transaction<'_, Postgres>, Transaction<'static, Postgres>>(tx)
        })),
        tenant_db,
        user_id: None, // Will be populated by auth middleware
    };
    
    request.extensions_mut().insert(ctx);
    
    // Execute handler
    let result = next.run(request).await;
    
    // Check response for success/error
    match result.status() {
        status if status.is_success() => {
            // Commit transaction on success
            if let Err(e) = tx.commit().await {
                error!("Failed to commit transaction: {}", e);
                return Err(ApiError::database_error("Transaction commit failed"));
            }
            info!("Transaction committed successfully");
            Ok(result)
        }
        _ => {
            // Rollback transaction on error
            if let Err(e) = tx.rollback().await {
                warn!("Failed to rollback transaction: {}", e);
            } else {
                info!("Transaction rolled back due to error response");
            }
            Ok(result) // Return the error response
        }
    }
}

/// Middleware for read-only operations - no transaction needed
/// Equivalent to TypeScript's withParams()
pub async fn with_context(
    Extension(pool): Extension<PgPool>,
    Extension(tenant_db): Extension<String>,
    mut request: Request,
    next: Next,
) -> Response {
    let ctx = RequestContext {
        pool,
        transaction: None,
        tenant_db,
        user_id: None,
    };
    
    request.extensions_mut().insert(ctx);
    next.run(request).await
}
```

### 3. API Response Types (100% Compatible)

```rust
// src/api/response.rs
use serde::{Deserialize, Serialize};
use axum::{response::{IntoResponse, Response}, Json, http::StatusCode};

/// Success response - matches TypeScript ApiSuccessResponse
#[derive(Debug, Serialize)]
pub struct ApiSuccessResponse<T> {
    pub success: bool,
    pub data: T,
}

/// Error response - matches TypeScript ApiErrorResponse
#[derive(Debug, Serialize)]
pub struct ApiErrorResponse {
    pub success: bool,
    pub error: String,
    pub error_code: ApiErrorCode,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<serde_json::Value>,
}

/// Error codes - matches TypeScript exactly
#[derive(Debug, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ApiErrorCode {
    ValidationError,
    NotFound,
    DependencyError,
    SchemaError,
    DatabaseError,
    InternalError,
}

/// Unified API response type
#[derive(Debug)]
pub enum ApiResponse<T> {
    Success(T),
    Error(ApiErrorResponse),
}

impl<T> ApiResponse<T> 
where 
    T: Serialize 
{
    /// Create success response
    pub fn success(data: T) -> Self {
        Self::Success(data)
    }
    
    /// Create error response
    pub fn error(error: impl Into<String>, error_code: ApiErrorCode) -> Self {
        Self::Error(ApiErrorResponse {
            success: false,
            error: error.into(),
            error_code,
            data: None,
        })
    }
    
    /// Create error response with additional data
    pub fn error_with_data(
        error: impl Into<String>, 
        error_code: ApiErrorCode,
        data: serde_json::Value
    ) -> Self {
        Self::Error(ApiErrorResponse {
            success: false,
            error: error.into(),
            error_code,
            data: Some(data),
        })
    }
}

impl<T> IntoResponse for ApiResponse<T> 
where 
    T: Serialize 
{
    fn into_response(self) -> Response {
        match self {
            Self::Success(data) => {
                let response = ApiSuccessResponse { 
                    success: true, 
                    data 
                };
                (StatusCode::OK, Json(response)).into_response()
            }
            Self::Error(error) => {
                let status = match error.error_code {
                    ApiErrorCode::NotFound => StatusCode::NOT_FOUND,
                    ApiErrorCode::ValidationError => StatusCode::BAD_REQUEST,
                    ApiErrorCode::DependencyError => StatusCode::CONFLICT,
                    ApiErrorCode::DatabaseError | ApiErrorCode::InternalError => StatusCode::INTERNAL_SERVER_ERROR,
                    ApiErrorCode::SchemaError => StatusCode::BAD_REQUEST,
                };
                (status, Json(error)).into_response()
            }
        }
    }
}
```

### 4. Error Handling

```rust
// src/api/error.rs
use thiserror::Error;
use crate::api::response::{ApiResponse, ApiErrorCode};

#[derive(Error, Debug)]
pub enum ApiError {
    #[error("Validation error: {0}")]
    Validation(String),
    
    #[error("Not found: {0}")]
    NotFound(String),
    
    #[error("Database error: {0}")]
    Database(#[from] sqlx::Error),
    
    #[error("Filter error: {0}")]
    Filter(#[from] crate::filter::FilterError),
    
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),
    
    #[error("Internal error: {0}")]
    Internal(String),
}

impl ApiError {
    pub fn validation_error(msg: impl Into<String>) -> Self {
        Self::Validation(msg.into())
    }
    
    pub fn not_found(msg: impl Into<String>) -> Self {
        Self::NotFound(msg.into())
    }
    
    pub fn database_error(msg: impl Into<String>) -> Self {
        Self::Internal(msg.into()) // Wrap as internal to avoid exposing DB details
    }
    
    pub fn internal_error(msg: impl Into<String>) -> Self {
        Self::Internal(msg.into())
    }
}

impl<T> From<ApiError> for ApiResponse<T>
where 
    T: serde::Serialize 
{
    fn from(error: ApiError) -> Self {
        match error {
            ApiError::Validation(msg) => {
                ApiResponse::error(msg, ApiErrorCode::ValidationError)
            }
            ApiError::NotFound(msg) => {
                ApiResponse::error(msg, ApiErrorCode::NotFound)
            }
            ApiError::Database(e) => {
                tracing::error!("Database error: {}", e);
                ApiResponse::error("Database operation failed", ApiErrorCode::DatabaseError)
            }
            ApiError::Filter(e) => {
                ApiResponse::error(format!("Filter error: {}", e), ApiErrorCode::ValidationError)
            }
            ApiError::Json(e) => {
                ApiResponse::error(format!("JSON parsing error: {}", e), ApiErrorCode::ValidationError)
            }
            ApiError::Internal(msg) => {
                tracing::error!("Internal error: {}", msg);
                ApiResponse::error("Internal server error", ApiErrorCode::InternalError)
            }
        }
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> axum::response::Response {
        let response: ApiResponse<()> = self.into();
        response.into_response()
    }
}
```

### 5. Repository Integration

```rust
// Update Repository to use RequestContext
impl<T> Repository<T> {
    pub fn new(table_name: impl Into<String>, ctx: &RequestContext) -> Self {
        Self {
            table_name: table_name.into(),
            db_context: ctx.db(),
            _phantom: std::marker::PhantomData,
        }
    }
    
    /// Select with automatic transaction support
    pub async fn select_any(&self, filter_data: FilterData) -> Result<Vec<T>, ApiError> {
        QueryBuilder::<T>::new(&self.table_name)
            .filter(filter_data)?
            .select_all(&self.db_context) // Uses transaction if available
            .await
            .map_err(ApiError::Database)
    }
    
    // All other methods work the same way...
}
```

### 6. Route Handler Examples

```rust
// src/handlers/protected/data/schema_post.rs
use crate::api::{ApiResponse, ApiError, RequestContext};

/// Create records in schema - uses transaction
pub async fn schema_post(
    Path(schema): Path<String>,
    Extension(ctx): Extension<RequestContext>,
    Json(records): Json<Vec<serde_json::Value>>,
) -> Result<ApiResponse<Vec<serde_json::Map<String, serde_json::Value>>>, ApiError> {
    
    // Create repository with transaction context
    let repo = DynamicRepository::new(&schema, &ctx);
    
    // All operations use the same transaction
    let mut created_records = Vec::new();
    for record_data in records {
        let created = repo.create(record_data).await?;
        created_records.push(created);
    }
    
    // Update related records in the same transaction
    if !created_records.is_empty() {
        let related_updates = repo.update_related_records(&created_records).await?;
        tracing::info!("Updated {} related records", related_updates);
    }
    
    // Return success - transaction will auto-commit
    Ok(ApiResponse::success(created_records))
    // Any error causes auto-rollback via middleware
}

// Read-only operation - no transaction needed
pub async fn schema_get(
    Path(schema): Path<String>,
    Extension(ctx): Extension<RequestContext>,
    Json(filter_data): Json<FilterData>,
) -> Result<ApiResponse<Vec<serde_json::Map<String, serde_json::Value>>>, ApiError> {
    
    let repo = DynamicRepository::new(&schema, &ctx);
    let records = repo.select_any(filter_data).await?;
    
    Ok(ApiResponse::success(records))
}
```

### 7. Router Setup

```rust
// src/main.rs - Router configuration
use crate::api::middleware::{with_transaction, with_context};

let app = Router::new()
    // Read-only routes use context middleware only
    .route("/api/data/:schema", get(protected::data::schema_get))
    .route("/api/find/:schema", post(protected::data::find_post))
    .layer(middleware::from_fn(with_context))
    
    // Write routes use transaction middleware  
    .route("/api/data/:schema", post(protected::data::schema_post))
    .route("/api/data/:schema", put(protected::data::schema_put))
    .route("/api/data/:schema", delete(protected::data::schema_delete))
    .route("/api/meta/:schema", post(protected::meta::schema_post))
    .route("/api/bulk", post(protected::bulk::bulk_post))
    .layer(middleware::from_fn(with_transaction))
    
    // JWT and system middleware applied to all protected routes
    .layer(jwt_auth_middleware())
    .layer(system_context_middleware());
```

## Key Advantages of Rust Design

### 1. **Automatic Transaction Management**
- **Middleware-based**: Transactions managed at framework level, not in business logic
- **Error-safe**: Automatic rollback on any error, impossible to forget cleanup
- **Resource management**: Rust's ownership system ensures connections are always released

### 2. **100% Response Compatibility**
```rust
// TypeScript output
{ "success": true, "data": [...] }
{ "success": false, "error": "Not found", "error_code": "NOT_FOUND" }

// Rust output (identical)
{ "success": true, "data": [...] }
{ "success": false, "error": "Not found", "error_code": "NOT_FOUND" }
```

### 3. **Superior Error Handling**
- **Structured errors**: thiserror provides rich error context
- **Compile-time safety**: `Result<T, E>` prevents unhandled errors
- **Automatic conversion**: ApiError → ApiResponse → HTTP response

### 4. **Performance Benefits**
- **Zero-cost transactions**: No runtime overhead for transaction context
- **Connection efficiency**: Better connection pool utilization
- **Memory safety**: No garbage collection, predictable resource usage

### 5. **Type Safety**
```rust
// Compile-time verification of response types
fn handler() -> Result<ApiResponse<User>, ApiError> {
    Ok(ApiResponse::success(user)) // ✅ Correct type
    // Ok(ApiResponse::success("string")) // ❌ Compile error
}
```

## Implementation Roadmap

1. **Core Types** (`api/response.rs`, `api/error.rs`)
   - ApiResponse and ApiError with 100% TypeScript compatibility
   - IntoResponse implementations for Axum integration

2. **Request Context** (`api/context.rs`)
   - RequestContext with optional transaction support
   - DatabaseContext enum for unified query execution

3. **Transaction Middleware** (`api/middleware/transaction.rs`)
   - with_transaction for write operations
   - with_context for read operations
   - Automatic BEGIN/COMMIT/ROLLBACK management

4. **Repository Updates** (`database/repository.rs`)
   - Update all repository methods to use RequestContext
   - Automatic transaction awareness in all database operations

5. **Handler Migration**
   - Update all handler signatures to use RequestContext
   - Apply appropriate middleware to route groups
   - Maintain identical JSON response formats

This design provides the same transaction semantics and response formats as the TypeScript implementation while adding compile-time safety, better performance, and automatic resource management.