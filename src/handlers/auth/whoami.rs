// handlers/auth/whoami.rs - GET /api/auth/whoami handler
// Equivalent to monk-api/src/routes/auth/whoami/GET.ts

use axum::{http::StatusCode, response::Json};
use serde_json::{json, Value};

/**
 * GET /api/auth/whoami - Get current authenticated user details
 * 
 * This is equivalent to your TypeScript handler:
 * export default withParams(async (context, { system, user }) => { ... })
 * 
 * In Rust, we use async functions that return Axum response types.
 * Path parameters and request bodies are extracted via function parameters.
 * 
 * @returns JSON with current user information
 */
pub async fn whoami_get() -> (StatusCode, Json<Value>) {
    // TODO: Extract JWT claims to get user information
    // TODO: Query database for full user details
    // TODO: Return user profile data
    
    // Placeholder response - mirrors your TypeScript structure
    (
        StatusCode::NOT_IMPLEMENTED,
        Json(json!({
            "error": "Auth whoami endpoint not yet implemented",
            "message": "This will return current user information from JWT token",
            "planned_response": {
                "id": "user_uuid",
                "username": "admin", 
                "email": "user@example.com",
                "tenant": "tenant_name",
                "role": "admin",
                "created_at": "2025-01-01T00:00:00Z"
            }
        }))
    )
}

/*
RUST HANDLER PATTERNS:

1. **Function Signature**: 
   - pub async fn name() -> ResponseType
   - pub makes it available to other modules
   - async allows database calls and other async operations
   
2. **Parameters** (will add later):
   - Path(schema): Extract :schema from URL 
   - Query(params): Extract query string parameters
   - Json(body): Extract JSON request body
   - Extension(user): Extract user from middleware (like your context.user)
   
3. **Return Types**:
   - Json<Value>: Simple JSON response (200 OK)
   - (StatusCode, Json<Value>): Custom status code + JSON
   - Result<Json<Value>, AppError>: Error handling with custom error types
   
4. **Error Handling**:
   - ? operator for propagating errors
   - Custom error types that implement IntoResponse
   - Much more compile-time safety than TypeScript!
*/