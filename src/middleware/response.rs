use axum::{
    response::{IntoResponse, Json, Response},
    http::StatusCode,
};
use serde::Serialize;
use serde_json::{json, Value};

/// Wrapper for API responses that automatically adds success envelope
#[derive(Debug)]
pub struct ApiResponse<T: Serialize> {
    pub data: T,
    pub status_code: Option<StatusCode>,
}

impl<T: Serialize> ApiResponse<T> {
    /// Create a successful API response with default 200 status
    pub fn success(data: T) -> Self {
        Self {
            data,
            status_code: None, // Default to 200 OK
        }
    }

    /// Create an API response with custom status code
    pub fn with_status(data: T, status_code: StatusCode) -> Self {
        Self {
            data,
            status_code: Some(status_code),
        }
    }

    /// Create a 201 Created response
    pub fn created(data: T) -> Self {
        Self::with_status(data, StatusCode::CREATED)
    }

    /// Create a 202 Accepted response  
    pub fn accepted(data: T) -> Self {
        Self::with_status(data, StatusCode::ACCEPTED)
    }

    /// Create a 204 No Content response (data will be ignored)
    pub fn no_content() -> ApiResponse<()> {
        ApiResponse::with_status((), StatusCode::NO_CONTENT)
    }
}

impl<T: Serialize> IntoResponse for ApiResponse<T> {
    fn into_response(self) -> Response {
        let status = self.status_code.unwrap_or(StatusCode::OK);
        
        // For 204 No Content, return empty response
        if status == StatusCode::NO_CONTENT {
            return status.into_response();
        }

        // Convert data to JSON Value for consistent envelope format
        let data_value = match serde_json::to_value(&self.data) {
            Ok(value) => value,
            Err(e) => {
                tracing::error!("Failed to serialize response data: {}", e);
                return (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(json!({
                        "success": false,
                        "error": "Failed to serialize response data"
                    }))
                ).into_response();
            }
        };

        // Wrap in success envelope
        let envelope = json!({
            "success": true,
            "data": data_value
        });

        (status, Json(envelope)).into_response()
    }
}

/// Convenience trait for easy conversion to ApiResponse
pub trait IntoApiResponse<T: Serialize> {
    fn into_api_response(self) -> ApiResponse<T>;
}

impl<T: Serialize> IntoApiResponse<T> for T {
    fn into_api_response(self) -> ApiResponse<T> {
        ApiResponse::success(self)
    }
}

// Convenience type aliases
pub type ApiSuccess<T> = ApiResponse<T>;
pub type ApiResult<T> = Result<ApiResponse<T>, crate::error::ApiError>;