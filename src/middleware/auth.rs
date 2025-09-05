use axum::{
    extract::{Request},
    http::{HeaderMap, StatusCode},
    middleware::Next,
    response::{IntoResponse, Json, Response},
};
use jsonwebtoken::{decode, DecodingKey, Validation};
use uuid::Uuid;

use crate::auth::Claims;
use crate::config;
use crate::error::ApiError;

/// Authenticated user context extracted from JWT
#[derive(Clone, Debug)]
pub struct AuthUser {
    pub tenant: String,
    pub user: String,
    pub database: String,
    pub access: String,
    pub user_id: Uuid,
}

impl From<Claims> for AuthUser {
    fn from(claims: Claims) -> Self {
        Self {
            tenant: claims.tenant,
            user: claims.user,
            database: claims.database,
            access: claims.access,
            user_id: claims.user_id,
        }
    }
}

/// JWT authentication middleware that validates tokens and extracts user context
pub async fn jwt_auth_middleware(
    headers: HeaderMap,
    mut request: Request,
    next: Next,
) -> Result<Response, impl IntoResponse> {
    // Extract JWT from Authorization header
    let token = extract_jwt_from_headers(&headers)
        .map_err(|msg| {
            let api_error = ApiError::unauthorized(msg);
            (
                StatusCode::from_u16(api_error.status_code()).unwrap(),
                Json(api_error.to_json()),
            )
        })?;

    // Validate and decode JWT
    let claims = validate_jwt(&token)
        .map_err(|msg| {
            let api_error = ApiError::unauthorized(msg);
            (
                StatusCode::from_u16(api_error.status_code()).unwrap(),
                Json(api_error.to_json()),
            )
        })?;

    // Convert claims to AuthUser and inject into request
    let auth_user = AuthUser::from(claims);
    request.extensions_mut().insert(auth_user);

    Ok(next.run(request).await)
}

/// Extract JWT token from Authorization header
fn extract_jwt_from_headers(headers: &HeaderMap) -> Result<String, String> {
    let auth_header = headers
        .get("authorization")
        .or_else(|| headers.get("Authorization"))
        .ok_or_else(|| "Missing Authorization header".to_string())?;

    let auth_str = auth_header
        .to_str()
        .map_err(|_| "Invalid Authorization header format".to_string())?;

    if let Some(token) = auth_str.strip_prefix("Bearer ") {
        if token.trim().is_empty() {
            return Err("Empty JWT token".to_string());
        }
        Ok(token.to_string())
    } else {
        Err("Authorization header must use Bearer token format".to_string())
    }
}

/// Validate JWT token and extract claims
fn validate_jwt(token: &str) -> Result<Claims, String> {
    let secret = &config::config().security.jwt_secret;
    
    if secret.is_empty() {
        return Err("JWT secret not configured".to_string());
    }

    let decoding_key = DecodingKey::from_secret(secret.as_bytes());
    let validation = Validation::default();

    let token_data = decode::<Claims>(token, &decoding_key, &validation)
        .map_err(|e| format!("Invalid JWT token: {}", e))?;

    Ok(token_data.claims)
}