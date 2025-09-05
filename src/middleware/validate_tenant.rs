use axum::{
    extract::{Extension, Request},
    http::StatusCode,
    middleware::Next,
    response::{IntoResponse, Json, Response},
};
use sqlx::{PgPool, Row};
use uuid::Uuid;

use crate::database::manager::DatabaseManager;
use crate::error::ApiError;
use super::auth::AuthUser;

/// Extracted tenant database pool, injected by middleware
#[derive(Clone)]
pub struct TenantPool(pub PgPool);

/// Validated tenant information from monk_main.tenants
#[derive(Clone, Debug)]
pub struct ValidatedTenant {
    pub id: Uuid,
    pub name: String,
    pub database: String,
    pub host: String,
    pub is_active: bool,
    pub tenant_type: String,
    pub access_read: Vec<Uuid>,
    pub access_edit: Vec<Uuid>,
    pub access_full: Vec<Uuid>,
    pub access_deny: Vec<Uuid>,
}

/// Middleware that validates the tenant from JWT claims against monk_main.tenants
/// Ensures the tenant exists and is active (not trashed/deleted)
pub async fn validate_tenant_middleware(
    mut request: Request,
    next: Next,
) -> Result<Response, impl IntoResponse> {
    // Get AuthUser from previous JWT middleware
    let auth_user = request.extensions().get::<AuthUser>()
        .ok_or_else(|| {
            let api_error = ApiError::unauthorized("JWT authentication required before tenant validation");
            (
                StatusCode::from_u16(api_error.status_code()).unwrap(),
                Json(api_error.to_json()),
            )
        })?
        .clone();

    // Query monk_main database to validate tenant
    let main_pool = DatabaseManager::main_pool().await
        .map_err(|e| {
            let api_error: ApiError = e.into();
            (
                StatusCode::from_u16(api_error.status_code()).unwrap(),
                Json(api_error.to_json()),
            )
        })?;

    // Query tenant by database name from JWT claims
    let query = r#"
        SELECT 
            id, name, database, host, is_active, tenant_type,
            access_read, access_edit, access_full, access_deny
        FROM tenants 
        WHERE database = $1 
        AND is_active = true
        AND trashed_at IS NULL 
        AND deleted_at IS NULL
    "#;

    let row = sqlx::query(query)
        .bind(&auth_user.database)
        .fetch_optional(&main_pool)
        .await
        .map_err(|e| {
            tracing::error!("Database error validating tenant: {}", e);
            let api_error = ApiError::internal_server_error("Failed to validate tenant");
            (
                StatusCode::from_u16(api_error.status_code()).unwrap(),
                Json(api_error.to_json()),
            )
        })?;

    let tenant_row = row.ok_or_else(|| {
        tracing::warn!("Tenant validation failed: tenant '{}' not found or inactive", auth_user.database);
        let api_error = ApiError::forbidden(format!("Tenant '{}' is not active or does not exist", auth_user.tenant));
        (
            StatusCode::from_u16(api_error.status_code()).unwrap(),
            Json(api_error.to_json()),
        )
    })?;

    // Extract tenant information
    let validated_tenant = ValidatedTenant {
        id: tenant_row.get("id"),
        name: tenant_row.get("name"),
        database: tenant_row.get("database"),
        host: tenant_row.get("host"),
        is_active: tenant_row.get("is_active"),
        tenant_type: tenant_row.get("tenant_type"),
        access_read: tenant_row.get("access_read"),
        access_edit: tenant_row.get("access_edit"),
        access_full: tenant_row.get("access_full"),
        access_deny: tenant_row.get("access_deny"),
    };

    tracing::debug!("Tenant validation successful: {} ({})", validated_tenant.name, validated_tenant.database);

    // Get database pool for the validated tenant
    let tenant_pool = DatabaseManager::tenant_pool(&validated_tenant.database)
        .await
        .map_err(|e| {
            tracing::error!("Failed to get database pool for tenant '{}': {}", validated_tenant.database, e);
            let api_error: ApiError = e.into();
            (
                StatusCode::from_u16(api_error.status_code()).unwrap(),
                Json(api_error.to_json()),
            )
        })?;

    tracing::debug!("Tenant database pool acquired for: {}", validated_tenant.database);

    // Inject both validated tenant and tenant pool into request
    request.extensions_mut().insert(validated_tenant);
    request.extensions_mut().insert(TenantPool(tenant_pool));

    Ok(next.run(request).await)
}