use serde_json::{Value, json};
use sqlx::Row;

use crate::api::format::{record_to_api_value, MetadataOptions};
use crate::database::manager::{DatabaseError, DatabaseManager};
use crate::filter::{Filter, FilterData};
use crate::observer::stateful_record::{RecordOperation, StatefulRecord};

/// Execute a SELECT operation through the (future) observer pipeline.
/// For now, this performs no-op rings and executes the enhanced query directly,
/// returning JSON:API-formatted records using the provided metadata options.
pub async fn execute_select(
    schema: &str,
    tenant_db: &str,
    filter_data: FilterData,
    meta_options: &MetadataOptions,
) -> Result<Vec<Value>, DatabaseError> {
    // PHASE 1: Query preparation (rings 0-4) - no-op for now.
    let mut effective_filter = filter_data;

    // Build SQL from Filter
    let mut filter = Filter::new(schema).map_err(|e| DatabaseError::QueryError(e.to_string()))?;
    filter
        .assign(effective_filter)
        .map_err(|e| DatabaseError::QueryError(e.to_string()))?;
    let sql_result = filter
        .to_sql()
        .map_err(|e| DatabaseError::QueryError(e.to_string()))?;

    // Wrap to return row_to_json rows like existing handlers
    let wrapped = format!("SELECT row_to_json(t) AS row FROM ({}) t", sql_result.query);

    // PHASE 2: Database execution (ring 5)
    let pool = DatabaseManager::tenant_pool(tenant_db).await?;

    let mut q = sqlx::query(&wrapped);
    for p in sql_result.params.iter() {
        q = bind_param(q, p);
    }

    let rows = q.fetch_all(&pool).await?;

    // Convert to StatefulRecords and extract system metadata
    let mut records = Vec::new();
    for row in rows {
        let v: Value = row.try_get("row").unwrap_or(Value::Null);
        if let Value::Object(map) = v {
            let mut rec = StatefulRecord::existing(map.clone(), None, RecordOperation::NoChange);
            rec.extract_system_metadata();
            records.push(rec);
        }
    }

    // PHASE 3: Post-database result processing (rings 6-9) - no-op for now.

    // Format as JSON:API resources
    let data: Vec<Value> = records
        .iter()
        .map(|r| record_to_api_value(r, schema, meta_options))
        .collect();

    Ok(data)
}

fn bind_param<'q>(
    q: sqlx::query::Query<'q, sqlx::Postgres, sqlx::postgres::PgArguments>,
    v: &'q Value,
) -> sqlx::query::Query<'q, sqlx::Postgres, sqlx::postgres::PgArguments> {
    match v {
        Value::Null => {
            let none: Option<String> = None;
            q.bind(none)
        }
        Value::Bool(b) => q.bind(*b),
        Value::Number(n) => {
            if let Some(i) = n.as_i64() { q.bind(i) }
            else if let Some(u) = n.as_u64() { q.bind(u as i64) }
            else if let Some(f) = n.as_f64() { q.bind(f) }
            else { q.bind(n.to_string()) }
        }
        Value::String(s) => q.bind(s),
        Value::Array(_arr) => {
            // Arrays should be expanded before binding; pass-through
            q
        }
        Value::Object(_) => q.bind(v.clone()), // JSONB
    }
}
