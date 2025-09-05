// HTTP API Error Types
use axum::{response::IntoResponse, http::StatusCode, Json};
use serde_json::{json, Value};
use std::collections::HashMap;

/// HTTP API error with appropriate status codes and client-friendly messages
#[derive(Debug)]
pub enum ApiError {
    // 400 Bad Request
    BadRequest(String),
    ValidationError { 
        message: String, 
        field_errors: Option<HashMap<String, String>> 
    },
    InvalidJson(String),
    
    // 401 Unauthorized  
    Unauthorized(String),
    
    // 403 Forbidden
    Forbidden(String),
    
    // 404 Not Found
    NotFound(String),
    
    // 409 Conflict
    Conflict(String),
    
    // 422 Unprocessable Entity (validation but semantically valid JSON)
    UnprocessableEntity { 
        message: String, 
        field_errors: HashMap<String, String> 
    },
    
    // 429 Too Many Requests
    TooManyRequests(String),
    
    // 500 Internal Server Error
    InternalServerError(String),
    
    // 502 Bad Gateway (external service issues)
    BadGateway(String),
    
    // 503 Service Unavailable  
    ServiceUnavailable(String),
}

impl ApiError {
    /// Get HTTP status code
    pub fn status_code(&self) -> u16 {
        match self {
            ApiError::BadRequest(_) => 400,
            ApiError::ValidationError { .. } => 400,
            ApiError::InvalidJson(_) => 400,
            ApiError::Unauthorized(_) => 401,
            ApiError::Forbidden(_) => 403,
            ApiError::NotFound(_) => 404,
            ApiError::Conflict(_) => 409,
            ApiError::UnprocessableEntity { .. } => 422,
            ApiError::TooManyRequests(_) => 429,
            ApiError::InternalServerError(_) => 500,
            ApiError::BadGateway(_) => 502,
            ApiError::ServiceUnavailable(_) => 503,
        }
    }
    
    /// Get client-safe error message
    pub fn message(&self) -> &str {
        match self {
            ApiError::BadRequest(msg) => msg,
            ApiError::ValidationError { message, .. } => message,
            ApiError::InvalidJson(msg) => msg,
            ApiError::Unauthorized(msg) => msg,
            ApiError::Forbidden(msg) => msg,
            ApiError::NotFound(msg) => msg,
            ApiError::Conflict(msg) => msg,
            ApiError::UnprocessableEntity { message, .. } => message,
            ApiError::TooManyRequests(msg) => msg,
            ApiError::InternalServerError(msg) => msg,
            ApiError::BadGateway(msg) => msg,
            ApiError::ServiceUnavailable(msg) => msg,
        }
    }
    
    /// Convert to JSON response body
    pub fn to_json(&self) -> Value {
        match self {
            ApiError::ValidationError { message, field_errors } => {
                let mut response = json!({
                    "error": true,
                    "message": message,
                    "code": "VALIDATION_ERROR"
                });
                
                if let Some(field_errors) = field_errors {
                    response["field_errors"] = json!(field_errors);
                }
                
                response
            }
            ApiError::UnprocessableEntity { message, field_errors } => {
                json!({
                    "error": true,
                    "message": message,
                    "code": "UNPROCESSABLE_ENTITY",
                    "field_errors": field_errors
                })
            }
            _ => {
                json!({
                    "error": true,
                    "message": self.message(),
                    "code": self.error_code()
                })
            }
        }
    }
    
    /// Get error code for client handling
    pub fn error_code(&self) -> &'static str {
        match self {
            ApiError::BadRequest(_) => "BAD_REQUEST",
            ApiError::ValidationError { .. } => "VALIDATION_ERROR",
            ApiError::InvalidJson(_) => "INVALID_JSON",
            ApiError::Unauthorized(_) => "UNAUTHORIZED", 
            ApiError::Forbidden(_) => "FORBIDDEN",
            ApiError::NotFound(_) => "NOT_FOUND",
            ApiError::Conflict(_) => "CONFLICT",
            ApiError::UnprocessableEntity { .. } => "UNPROCESSABLE_ENTITY",
            ApiError::TooManyRequests(_) => "TOO_MANY_REQUESTS",
            ApiError::InternalServerError(_) => "INTERNAL_SERVER_ERROR",
            ApiError::BadGateway(_) => "BAD_GATEWAY",
            ApiError::ServiceUnavailable(_) => "SERVICE_UNAVAILABLE",
        }
    }
}

// Static constructor methods (similar to TypeScript HttpErrors class)
impl ApiError {
    pub fn bad_request(message: impl Into<String>) -> Self {
        ApiError::BadRequest(message.into())
    }
    
    pub fn validation_error(
        message: impl Into<String>, 
        field_errors: Option<HashMap<String, String>>
    ) -> Self {
        ApiError::ValidationError { 
            message: message.into(), 
            field_errors 
        }
    }
    
    pub fn invalid_json(message: impl Into<String>) -> Self {
        ApiError::InvalidJson(message.into())
    }
    
    pub fn unauthorized(message: impl Into<String>) -> Self {
        ApiError::Unauthorized(message.into())
    }
    
    pub fn forbidden(message: impl Into<String>) -> Self {
        ApiError::Forbidden(message.into())
    }
    
    pub fn not_found(message: impl Into<String>) -> Self {
        ApiError::NotFound(message.into())
    }
    
    pub fn conflict(message: impl Into<String>) -> Self {
        ApiError::Conflict(message.into())
    }
    
    pub fn unprocessable_entity(
        message: impl Into<String>, 
        field_errors: HashMap<String, String>
    ) -> Self {
        ApiError::UnprocessableEntity { 
            message: message.into(), 
            field_errors 
        }
    }
    
    pub fn too_many_requests(message: impl Into<String>) -> Self {
        ApiError::TooManyRequests(message.into())
    }
    
    pub fn internal_server_error(message: impl Into<String>) -> Self {
        ApiError::InternalServerError(message.into())
    }
    
    pub fn bad_gateway(message: impl Into<String>) -> Self {
        ApiError::BadGateway(message.into())
    }
    
    pub fn service_unavailable(message: impl Into<String>) -> Self {
        ApiError::ServiceUnavailable(message.into())
    }
}

// Convert other error types to ApiError
impl From<crate::database::record::RecordError> for ApiError {
    fn from(err: crate::database::record::RecordError) -> Self {
        match err {
            crate::database::record::RecordError::SystemFieldNotAllowed(field) => {
                ApiError::bad_request(format!("System field '{}' cannot be set via API", field))
            }
            crate::database::record::RecordError::InvalidJson(msg) => {
                ApiError::invalid_json(msg)
            }
            crate::database::record::RecordError::MissingRequiredField(field) => {
                let mut field_errors = HashMap::new();
                field_errors.insert(field.clone(), "This field is required".to_string());
                ApiError::validation_error(
                    "Missing required fields", 
                    Some(field_errors)
                )
            }
            crate::database::record::RecordError::InvalidUuid { field, value } => {
                let mut field_errors = HashMap::new();
                field_errors.insert(field, format!("Invalid UUID format: {}", value));
                ApiError::validation_error(
                    "Invalid field format", 
                    Some(field_errors)
                )
            }
            crate::database::record::RecordError::InvalidTimestamp { field, value } => {
                let mut field_errors = HashMap::new();
                field_errors.insert(field, format!("Invalid timestamp format: {}", value));
                ApiError::validation_error(
                    "Invalid field format", 
                    Some(field_errors)
                )
            }
        }
    }
}

impl From<crate::database::manager::DatabaseError> for ApiError {
    fn from(err: crate::database::manager::DatabaseError) -> Self {
        match err {
            crate::database::manager::DatabaseError::NotFound(msg) => {
                ApiError::not_found(msg)
            }
            crate::database::manager::DatabaseError::ConnectionError(_) => {
                ApiError::service_unavailable("Database temporarily unavailable")
            }
            crate::database::manager::DatabaseError::QueryError(msg) => {
                // Don't expose internal SQL errors to clients
                tracing::error!("Database query error: {}", msg);
                ApiError::internal_server_error("An error occurred while processing your request")
            }
            crate::database::manager::DatabaseError::Sqlx(sqlx_err) => {
                // Log the real error but return generic message
                tracing::error!("SQLx error: {}", sqlx_err);
                ApiError::internal_server_error("Database error occurred")
            }
            crate::database::manager::DatabaseError::MigrationError(msg) => {
                tracing::error!("Migration error: {}", msg);
                ApiError::service_unavailable("Service is being updated, please try again later")
            }
        }
    }
}

impl From<crate::observer::error::ObserverError> for ApiError {
    fn from(err: crate::observer::error::ObserverError) -> Self {
        match err {
            crate::observer::error::ObserverError::ValidationError(msg) => {
                ApiError::validation_error(msg, None)
            }
            crate::observer::error::ObserverError::NotFound(msg) => {
                ApiError::not_found(msg)
            }
            crate::observer::error::ObserverError::DatabaseError(msg) => {
                tracing::error!("Observer database error: {}", msg);
                ApiError::internal_server_error("An error occurred while processing your request")
            }
            crate::observer::error::ObserverError::TimeoutError(msg) => {
                tracing::error!("Observer timeout: {}", msg);
                ApiError::internal_server_error("Request processing timed out")
            }
        }
    }
}

impl<E: Into<ApiError>> From<crate::database::record::RecordResultError<E>> for ApiError {
    fn from(err: crate::database::record::RecordResultError<E>) -> Self {
        match err {
            crate::database::record::RecordResultError::OriginalError(e) => e.into(),
            crate::database::record::RecordResultError::SerializationError(e) => {
                tracing::error!("JSON serialization error: {}", e);
                ApiError::internal_server_error("Failed to format response")
            }
        }
    }
}

// Standard error trait implementations
impl std::fmt::Display for ApiError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.message())
    }
}

impl std::error::Error for ApiError {}

// Automatic HTTP response conversion for Axum
impl IntoResponse for ApiError {
    fn into_response(self) -> axum::response::Response {
        let status = StatusCode::from_u16(self.status_code()).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR);
        (status, Json(self.to_json())).into_response()
    }
}