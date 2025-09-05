// Ring 5: Select SQL Executor - handles SELECT operations
use async_trait::async_trait;
use serde_json::{Value, Map};
use sqlx::{PgPool, Row, Column, TypeInfo};
use uuid::Uuid;

use crate::observer::traits::{Observer, Ring5, ObserverRing, Operation};
use crate::observer::context::ObserverContext;
use crate::observer::error::ObserverError;
use crate::database::manager::DatabaseManager;
use crate::filter::Filter;

/// Ring 5: Select SQL Executor - handles SELECT operations only
#[derive(Default)]
pub struct SelectSqlExecutor;

impl Observer for SelectSqlExecutor {
    fn name(&self) -> &'static str { 
        "SelectSqlExecutor" 
    }
    
    fn ring(&self) -> ObserverRing { 
        ObserverRing::Database 
    }
    
    fn applies_to_operation(&self, op: Operation) -> bool {
        matches!(op, Operation::Select)
    }
    
    fn applies_to_schema(&self, _schema: &str) -> bool {
        true // Applies to all schemas
    }
}

#[async_trait]
impl Ring5 for SelectSqlExecutor {
    async fn execute(&self, ctx: &mut ObserverContext) -> Result<(), ObserverError> {
        let filter_data = ctx.filter_data.clone().unwrap_or_default();
        
        tracing::info!("Executing SELECT operation for schema: {}", ctx.schema_name);
        
        // Get database connection
        let pool = DatabaseManager::main_pool().await
            .map_err(|e| ObserverError::DatabaseError(e.to_string()))?;
        
        // Build SQL query using Filter system
        let mut filter = Filter::new(&ctx.schema_name)
            .map_err(|e| ObserverError::DatabaseError(e.to_string()))?;
        
        filter.assign(filter_data)
            .map_err(|e| ObserverError::DatabaseError(e.to_string()))?;
        
        let sql_result = filter.to_sql()
            .map_err(|e| ObserverError::DatabaseError(e.to_string()))?;
        
        // Execute query
        let query_start = std::time::Instant::now();
        
        let mut query = sqlx::query(&sql_result.query);
        for param in &sql_result.params {
            query = bind_param(query, param);
        }
        
        let rows = query.fetch_all(&pool).await
            .map_err(|e| ObserverError::DatabaseError(e.to_string()))?;
        
        let query_time = query_start.elapsed();
        
        // Convert raw results to Records for post-processing rings
        let mut records = Vec::new();
        let mut raw_results = Vec::new();
        
        for row in rows {
            // Extract row data as JSON
            let mut record_data = Map::new();
            
            // Convert row to Map<String, Value>
            for (i, column) in row.columns().iter().enumerate() {
                let column_name = column.name();
                let value = self.extract_column_value(&row, i, column.type_info())?;
                record_data.insert(column_name.to_string(), value);
            }
            
            // Create Record from SELECT result
            let record = crate::database::record::Record::from_sql_data(
                record_data.iter().map(|(k, v)| (k.clone(), v.clone())).collect()
            );
            records.push(record);
            
            // Also keep raw JSON for results
            raw_results.push(Value::Object(record_data));
        }
        
        // Update context with Records for post-processing
        ctx.records = records;
        
        // Store raw results
        ctx.result = Some(raw_results);
        
        tracing::info!(
            "SELECT executed: {} records returned in {}ms",
            ctx.records.len(),
            query_time.as_millis()
        );
        
        Ok(())
    }
}

impl SelectSqlExecutor {
    /// Extract typed value from database column
    fn extract_column_value(
        &self, 
        row: &sqlx::postgres::PgRow, 
        index: usize, 
        type_info: &sqlx::postgres::PgTypeInfo
    ) -> Result<Value, ObserverError> {
        let type_name = type_info.name();
        
        match type_name {
            "UUID" => {
                if let Ok(uuid) = row.try_get::<Option<Uuid>, _>(index) {
                    Ok(uuid.map(|u| Value::String(u.to_string())).unwrap_or(Value::Null))
                } else {
                    Ok(Value::Null)
                }
            }
            "TEXT" | "VARCHAR" => {
                if let Ok(text) = row.try_get::<Option<String>, _>(index) {
                    Ok(text.map(Value::String).unwrap_or(Value::Null))
                } else {
                    Ok(Value::Null)
                }
            }
            "INT4" | "INT8" => {
                if let Ok(num) = row.try_get::<Option<i64>, _>(index) {
                    Ok(num.map(|n| Value::Number(n.into())).unwrap_or(Value::Null))
                } else {
                    Ok(Value::Null)
                }
            }
            "BOOL" => {
                if let Ok(b) = row.try_get::<Option<bool>, _>(index) {
                    Ok(b.map(Value::Bool).unwrap_or(Value::Null))
                } else {
                    Ok(Value::Null)
                }
            }
            "JSONB" | "JSON" => {
                if let Ok(json) = row.try_get::<Option<Value>, _>(index) {
                    Ok(json.unwrap_or(Value::Null))
                } else {
                    Ok(Value::Null)
                }
            }
            "TIMESTAMPTZ" | "TIMESTAMP" => {
                if let Ok(ts) = row.try_get::<Option<chrono::DateTime<chrono::Utc>>, _>(index) {
                    Ok(ts.map(|t| Value::String(t.to_rfc3339())).unwrap_or(Value::Null))
                } else {
                    Ok(Value::Null)
                }
            }
            _ => {
                tracing::warn!("Unhandled PostgreSQL type: {}, falling back to string", type_name);
                Ok(Value::String(format!("<unsupported type: {}>", type_name)))
            }
        }
    }
}

/// Bind parameter to SQL query
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
        Value::Array(_arr) => {
            // Arrays should be expanded before binding; for now pass through as JSON
            q.bind(v)
        }
        Value::Object(_) => q.bind(v), // JSONB
    }
}