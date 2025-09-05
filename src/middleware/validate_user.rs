use axum::{
    extract::{Extension, Request},
    http::StatusCode,
    middleware::Next,
    response::{IntoResponse, Json, Response},
};
use sqlx::Row;
use uuid::Uuid;

use crate::error::ApiError;
use super::auth::AuthUser;
use super::validate_tenant::TenantPool;

/// Validated user information from tenant-specific users table
#[derive(Clone, Debug)]
pub struct ValidatedUser {
    pub id: Uuid,
    pub name: String,
    pub auth: String,
    pub access: String,
    pub access_read: Vec<Uuid>,
    pub access_edit: Vec<Uuid>,
    pub access_full: Vec<Uuid>,
    pub access_deny: Vec<Uuid>,
}

/// Middleware that validates the user from JWT claims against the tenant's users table
/// Ensures the user exists and is active (not trashed/deleted) in the tenant database
pub async fn validate_user_middleware(
    mut request: Request,
    next: Next,
) -> Result<Response, impl IntoResponse> {
    // Get AuthUser from JWT middleware
    let auth_user = request.extensions().get::<AuthUser>()
        .ok_or_else(|| {
            let api_error = ApiError::unauthorized("JWT authentication required before user validation");
            (
                StatusCode::from_u16(api_error.status_code()).unwrap(),
                Json(api_error.to_json()),
            )
        })?
        .clone();

    // Get TenantPool from tenant middleware
    let TenantPool(tenant_pool) = request.extensions().get::<TenantPool>()
        .ok_or_else(|| {
            let api_error = ApiError::internal_server_error("Tenant pool required before user validation");
            (
                StatusCode::from_u16(api_error.status_code()).unwrap(),
                Json(api_error.to_json()),
            )
        })?
        .clone();

    // Query user in tenant database by user ID from JWT claims
    let query = r#"
        SELECT 
            id, name, auth, access,
            access_read, access_edit, access_full, access_deny
        FROM users 
        WHERE id = $1 
        AND trashed_at IS NULL 
        AND deleted_at IS NULL
    "#;

    let row = sqlx::query(query)
        .bind(&auth_user.user_id)
        .fetch_optional(&tenant_pool)
        .await
        .map_err(|e| {
            tracing::error!("Database error validating user in tenant '{}': {}", auth_user.database, e);
            let api_error = ApiError::internal_server_error("Failed to validate user");
            (
                StatusCode::from_u16(api_error.status_code()).unwrap(),
                Json(api_error.to_json()),
            )
        })?;

    let user_row = row.ok_or_else(|| {
        tracing::warn!("User validation failed: user '{}' (ID: {}) not found or inactive in tenant '{}'", 
                      auth_user.user, auth_user.user_id, auth_user.database);
        let api_error = ApiError::forbidden(format!("User '{}' is not active in tenant '{}'", auth_user.user, auth_user.tenant));
        (
            StatusCode::from_u16(api_error.status_code()).unwrap(),
            Json(api_error.to_json()),
        )
    })?;

    // Verify that JWT claims match database record
    let db_auth: String = user_row.get("auth");
    if db_auth != auth_user.user {
        tracing::warn!("User validation failed: JWT user '{}' doesn't match database auth '{}'", 
                      auth_user.user, db_auth);
        let api_error = ApiError::forbidden("User authentication mismatch");
        return Err((
            StatusCode::from_u16(api_error.status_code()).unwrap(),
            Json(api_error.to_json()),
        ));
    }

    // Verify access level matches JWT claims
    let db_access: String = user_row.get("access");
    if db_access != auth_user.access {
        tracing::warn!("User validation failed: JWT access '{}' doesn't match database access '{}'", 
                      auth_user.access, db_access);
        let api_error = ApiError::forbidden("User access level mismatch");
        return Err((
            StatusCode::from_u16(api_error.status_code()).unwrap(),
            Json(api_error.to_json()),
        ));
    }

    // Check for deny access
    if db_access == "deny" {
        tracing::warn!("User validation failed: user '{}' has deny access", auth_user.user);
        let api_error = ApiError::forbidden("User access denied");
        return Err((
            StatusCode::from_u16(api_error.status_code()).unwrap(),
            Json(api_error.to_json()),
        ));
    }

    // Extract validated user information
    let validated_user = ValidatedUser {
        id: user_row.get("id"),
        name: user_row.get("name"),
        auth: user_row.get("auth"),
        access: user_row.get("access"),
        access_read: user_row.get("access_read"),
        access_edit: user_row.get("access_edit"),
        access_full: user_row.get("access_full"),
        access_deny: user_row.get("access_deny"),
    };

    tracing::debug!("User validation successful: {} ({}) with {} access in tenant '{}'", 
                   validated_user.name, validated_user.auth, validated_user.access, auth_user.tenant);

    // Inject validated user into request
    request.extensions_mut().insert(validated_user);

    Ok(next.run(request).await)
}