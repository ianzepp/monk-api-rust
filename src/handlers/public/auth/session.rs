use axum::{extract::Path, http::StatusCode, response::IntoResponse, Json};
use serde::Deserialize;
use serde_json::{json, Value};

use crate::auth::{generate_jwt, Claims};
use crate::database::service::{find_tenant_by_name, find_user_by_auth};

#[derive(Debug, Deserialize)]
pub struct LoginRequest {
    pub password: String,
}

#[derive(Debug, Deserialize)]
pub struct RefreshRequest {
    pub token: String,
}

/// POST /auth/login/:tenant/:user - Authenticate user and receive JWT token
///
/// This is the primary authentication endpoint that validates user credentials
/// and returns a JWT token for accessing protected APIs.
///
/// URL Parameters:
/// - tenant: Tenant name (from tenants.name column in monk_main DB)
/// - user: User auth identifier (from users.auth column in tenant DB)
///
/// Expected Input:
/// ```json
/// {
///   "password": "string"    // Required: User password
/// }
/// ```
///
/// Expected Output (Success):
/// ```json
/// {
///   "success": true,
///   "data": {
///     "token": "eyJhbGciOiJIUzI1NiI...",
///     "user": {
///       "id": "user_uuid",
///       "username": "admin",
///       "tenant": "my-tenant",
///       "database": "tenant_abc123",
///       "access": "full"
///     },
///     "expires_in": 3600
///   }
/// }
/// ```
pub async fn login(
    Path((tenant_name, user_auth)): Path<(String, String)>,
    Json(_payload): Json<LoginRequest>,
) -> impl IntoResponse {
    // 1. Check if tenant exists
    let tenant = match find_tenant_by_name(&tenant_name).await {
        Ok(Some(tenant)) => tenant,
        Ok(None) => {
            return (
                StatusCode::NOT_FOUND,
                Json(json!({
                    "success": false,
                    "error": "Tenant not found",
                    "error_code": "TENANT_NOT_FOUND"
                })),
            );
        }
        Err(e) => {
            tracing::error!("Database error checking tenant {}: {}", tenant_name, e);
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({
                    "success": false,
                    "error": "Database error",
                    "error_code": "DATABASE_ERROR"
                })),
            );
        }
    };

    // 2. Check if user exists in tenant database
    let user = match find_user_by_auth(&tenant.database, &user_auth).await {
        Ok(Some(user)) => user,
        Ok(None) => {
            return (
                StatusCode::NOT_FOUND,
                Json(json!({
                    "success": false,
                    "error": "User not found",
                    "error_code": "USER_NOT_FOUND"
                })),
            );
        }
        Err(e) => {
            tracing::error!(
                "Database error checking user {} in {}: {}",
                user_auth,
                tenant.database,
                e
            );
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({
                    "success": false,
                    "error": "Database error",
                    "error_code": "DATABASE_ERROR"
                })),
            );
        }
    };

    // 3. Generate JWT token
    let claims = Claims::new(
        tenant.name.clone(),
        user.auth.clone(),
        tenant.database.clone(),
        user.access.clone(),
        user.id,
    );

    let token = match generate_jwt(claims) {
        Ok(token) => token,
        Err(e) => {
            tracing::error!("JWT generation error: {}", e);
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({
                    "success": false,
                    "error": "Token generation failed",
                    "error_code": "JWT_ERROR"
                })),
            );
        }
    };

    // 4. Return success response
    let expires_in = crate::config::config().security.jwt_expiry_hours * 3600; // Convert to seconds

    (
        StatusCode::OK,
        Json(json!({
            "success": true,
            "data": {
                "token": token,
                "user": {
                    "id": user.id,
                    "database": tenant.database,
                    "tenant": tenant.name,
                    "auth": user.auth,
                    "name": user.name,
                    "access": user.access
                },
                "expires_in": expires_in
            }
        })),
    )
}

/// POST /auth/refresh/:tenant/:user - Refresh expired JWT token
///
/// Allows clients to refresh their JWT tokens without requiring full
/// re-authentication. Accepts an existing JWT token (which may be expired)
/// and returns a new token with extended expiration.
///
/// URL Parameters:
/// - tenant: Tenant name (from tenants.name column in monk_main DB)
/// - user: User auth identifier (from users.auth column in tenant DB)
///
/// Expected Input:
/// ```json
/// {
///   "token": "string"    // Required: Current JWT token (may be expired)
/// }
/// ```
///
/// Expected Output (Success):
/// ```json
/// {
///   "success": true,
///   "data": {
///     "token": "eyJhbGciOiJIUzI1NiI...",
///     "expires_in": 3600
///   }
/// }
/// ```
pub async fn refresh(
    Path((tenant, user)): Path<(String, String)>,
    Json(_payload): Json<RefreshRequest>,
) -> impl IntoResponse {
    // TODO: Validate tenant and user parameters match token claims
    // TODO: Extract JWT token from request body
    // TODO: Validate token signature using utils::validate_jwt_token (even if expired)
    // TODO: Check token hasn't been revoked/blacklisted
    // TODO: Extract user claims from existing token
    // TODO: Generate new JWT token with same claims but new expiration
    // TODO: Return new token to client

    (
        StatusCode::NOT_IMPLEMENTED,
        Json(json!({
            "success": false,
            "error": "Token refresh endpoint not yet implemented",
            "message": "This will refresh JWT tokens without full re-authentication",
            "expected_input": {
                "token": "string (required - existing JWT token)"
            },
            "planned_response": {
                "success": true,
                "data": {
                    "token": "eyJhbGciOiJIUzI1NiI...",
                    "expires_in": 3600
                }
            }
        })),
    )
}

/*
SESSION MANAGEMENT IMPLEMENTATION STRATEGY:

LOGIN FLOW:
1. **Request Validation**:
   - Validate tenant and username are provided
   - Sanitize input for security

2. **Credential Validation**:
   - Look up tenant in system database
   - Validate tenant is active and not suspended
   - Query user credentials in tenant-specific database
   - Verify password hash or authentication method

3. **JWT Generation**:
   ```rust
   let claims = JWTClaims {
       sub: user.id,
       tenant: tenant.name,
       database: tenant.database_name,
       access: user.role,
       exp: (Utc::now() + Duration::hours(24)).timestamp(),
   };
   ```

4. **Response Formation**:
   - Return JWT token in standardized format
   - Include user information for client-side use
   - Set appropriate cache headers
   - Log successful authentication for audit

REFRESH FLOW:
1. **Token Validation**:
   ```rust
   // Parse JWT even if expired (skip expiration validation)
   let claims = decode::<JWTClaims>(
       &token,
       &key,
       &Validation { validate_exp: false, ..Default::default() }
   )?;
   ```

2. **Security Checks**:
   - Verify token signature is valid (ensures authenticity)
   - Check token hasn't been explicitly revoked
   - Validate user still exists and is active
   - Check tenant is still active

3. **Token Refresh Policy**:
   - Only allow refresh within reasonable time window (e.g., 7 days after expiration)
   - Prevent refresh of tokens that are too old
   - May require shorter refresh window for elevated tokens

4. **New Token Generation**:
   - Preserve all original claims (user, tenant, permissions)
   - Update expiration timestamp
   - Generate new JWT with same signing key

ERROR HANDLING:
- 400 Bad Request: Missing tenant/username/token
- 401 Unauthorized: Invalid credentials/token signature
- 403 Forbidden: Suspended tenant/user or refresh window expired
- 410 Gone: User/tenant no longer exists
- 500 Internal Server Error: Database/system errors

SECURITY CONSIDERATIONS:
- Rate limiting to prevent brute force attacks
- Secure password hashing verification
- Comprehensive audit logging
- Input sanitization and validation
- Proper error messages that don't leak information
*/
