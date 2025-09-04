use axum::{extract::{Path, Query}, response::IntoResponse, Json};
use serde::Deserialize;
use serde_json::{json, Value};
use sqlx::{self, Row};

use crate::database::manager::DatabaseManager;
use crate::filter::{Filter, FilterData};
use crate::observer::stateful_record::{RecordOperation, StatefulRecord};
use crate::handlers::protected::data::utils;

#[derive(Debug, Deserialize)]
pub struct FindQuery {
    pub tenant: Option<String>,
    pub meta: Option<String>,
}

/// POST /api/find/:schema - advanced filtered find
pub async fn find_post(
    Path(schema): Path<String>,
    Query(query): Query<FindQuery>,
    Json(filter_data): Json<FilterData>,
) -> impl IntoResponse {
    // Resolve tenant database
    let tenant_db = match utils::resolve_tenant_db(&query.tenant) {
        Ok(db) => db,
        Err(msg) => return (axum::http::StatusCode::BAD_REQUEST, Json(json!({"success": false, "error": msg }))).into_response(),
    };

    // Prepare metadata options from query
    let options = utils::metadata_options_from_query(query.meta.as_deref());

    // Execute via (future) observer pipeline - currently no-op observers
    match crate::observer::pipeline::execute_select(&schema, &tenant_db, filter_data, &options).await {
        Ok(data) => Json(json!({ "success": true, "data": data })).into_response(),
        Err(e) => (
            axum::http::StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"success": false, "error": format!("select failed: {}", e) })),
        ).into_response(),
    }
}
