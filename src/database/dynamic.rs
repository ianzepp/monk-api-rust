//! Dynamic repository for runtime-created schemas
//! Minimal scaffold: focus is on filter-related read paths.

use serde_json::{Map, Value};
use sqlx::{Column, PgPool, Row};

use crate::database::manager::DatabaseError;
use crate::filter::types::SqlResult;
use crate::filter::{Filter, FilterData};

pub struct DynamicRepository {
    table_name: String,
    pool: PgPool,
}

impl DynamicRepository {
    pub fn new(table_name: impl Into<String>, pool: PgPool) -> Self {
        Self { table_name: table_name.into(), pool }
    }

    /// Select any records using the Filter language, returning raw JSON maps.
    pub async fn select_any(
        &self,
        filter_data: FilterData,
    ) -> Result<Vec<Map<String, Value>>, DatabaseError> {
        let sql_result = if filter_data.select.is_some()
            || filter_data.where_clause.is_some()
            || filter_data.order.is_some()
            || filter_data.limit.is_some()
            || filter_data.offset.is_some()
        {
            let mut filter = Filter::new(&self.table_name)
                .map_err(|e| DatabaseError::QueryError(e.to_string()))?;
            filter.assign(filter_data).map_err(|e| DatabaseError::QueryError(e.to_string()))?;
            filter.to_sql().map_err(|e| DatabaseError::QueryError(e.to_string()))?
        } else {
            SqlResult { query: format!("SELECT * FROM \"{}\"", self.table_name), params: vec![] }
        };

        // Execute with plain query and manual row->JSON conversion later (TODO)
        let mut q = sqlx::query(&sql_result.query);
        for p in sql_result.params.iter() {
            q = bind_param(q, p);
        }
        let rows = q.fetch_all(&self.pool).await?;

        // Convert rows to Map<String, Value>
        let mut results = Vec::new();
        for row in rows {
            let mut map = Map::new();
            let column_count = row.len();

            for i in 0..column_count {
                let column_name = row.column(i).name();
                let value: Result<Option<Value>, _> = row.try_get(i);

                let json_value = match value {
                    Ok(Some(v)) => v,
                    Ok(None) => Value::Null,
                    Err(_) => {
                        // Try different types if direct JSON extraction fails
                        if let Ok(s) = row.try_get::<String, _>(i) {
                            Value::String(s)
                        } else if let Ok(i64_val) = row.try_get::<i64, _>(i) {
                            Value::Number(i64_val.into())
                        } else if let Ok(f64_val) = row.try_get::<f64, _>(i) {
                            Value::Number(
                                serde_json::Number::from_f64(f64_val).unwrap_or_else(|| 0.into()),
                            )
                        } else if let Ok(bool_val) = row.try_get::<bool, _>(i) {
                            Value::Bool(bool_val)
                        } else {
                            Value::Null
                        }
                    }
                };

                map.insert(column_name.to_string(), json_value);
            }
            results.push(map);
        }

        Ok(results)
    }
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
            if let Some(i) = n.as_i64() {
                q.bind(i)
            } else if let Some(u) = n.as_u64() {
                q.bind(u as i64)
            } else if let Some(f) = n.as_f64() {
                q.bind(f)
            } else {
                q.bind(n.to_string())
            }
        }
        Value::String(s) => q.bind(s),
        Value::Array(_arr) => q,
        Value::Object(_) => q.bind(v.clone()),
    }
}
