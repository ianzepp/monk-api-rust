use axum::{
    extract::{Path, Query},
    http::StatusCode,
    response::{IntoResponse, Json},
};
use serde::Deserialize;
use serde_json::{json, Value};

use crate::filter::FilterData;
use crate::handlers::protected::data::utils;
use crate::observer::pipeline::execute_select;

#[derive(Debug, Deserialize)]
pub struct FindQuery {
    /// Tenant database name. If omitted, falls back to MONK_TENANT_DB env.
    pub tenant: Option<String>,
    /// Include metadata sections. Examples: meta=true, meta=system,permissions
    pub meta: Option<String>,
}

/// POST /api/find/:schema - Advanced filtered search
/// 
/// Accepts a FilterData JSON body with:
/// - select: fields to return
/// - where: filter conditions
/// - order: sort order
/// - limit/offset: pagination
pub async fn post(
    Path(schema): Path<String>,
    Query(query): Query<FindQuery>,
    Json(filter_data): Json<FilterData>,
) -> impl IntoResponse {
    // Resolve tenant database
    let tenant_db = match utils::resolve_tenant_db(&query.tenant) {
        Ok(db) => db,
        Err(msg) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(json!({"success": false, "error": msg })),
            )
                .into_response()
        }
    };

    // Prepare metadata options from query
    let options = utils::metadata_options_from_query(query.meta.as_deref());

    // Execute via observer pipeline
    match execute_select(&schema, &tenant_db, filter_data, &options).await {
        Ok(data) => Json(json!({ "success": true, "data": data })).into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "success": false, "error": e.to_string() })),
        )
            .into_response(),
    }
}

/// DELETE /api/find/:schema - Bulk delete matching records
/// 
/// Accepts FilterData for the search criteria
pub async fn delete(
    Path(schema): Path<String>,
    Query(query): Query<FindQuery>,
    Json(filter_data): Json<FilterData>,
) -> impl IntoResponse {
    let _tenant_db = match utils::resolve_tenant_db(&query.tenant) {
        Ok(db) => db,
        Err(msg) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(json!({"success": false, "error": msg })),
            )
        }
    };

    // TODO: Implement bulk delete for matching records
    // 1. Use filter criteria to find records
    // 2. Soft delete all matching records
    // 3. Return count of deleted records
    
    (
        StatusCode::NOT_IMPLEMENTED,
        Json(json!({
            "success": false,
            "error": format!("DELETE /api/find/{} not yet implemented", schema),
            "message": "This will bulk delete records matching the filter"
        })),
    )
}