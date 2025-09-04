// Ring 5: Update SQL Executor - handles UPDATE operations
use async_trait::async_trait;
use serde_json::{Value, Map};
use sqlx::{PgPool, Row, Column, TypeInfo};
use uuid::Uuid;

use crate::observer::traits::{Observer, DatabaseObserver, ObserverRing, Operation};
use crate::observer::context::ObserverContext;
use crate::observer::error::ObserverError;
use crate::observer::stateful_record::{StatefulRecord, SqlOperation};
use crate::database::manager::DatabaseManager;

/// Ring 5: Update SQL Executor - handles UPDATE operations only
#[derive(Default)]
pub struct UpdateSqlExecutor;

impl Observer for UpdateSqlExecutor {
    fn name(&self) -> &'static str { 
        "UpdateSqlExecutor" 
    }
    
    fn ring(&self) -> ObserverRing { 
        ObserverRing::Database 
    }
    
    fn applies_to_operation(&self, op: Operation) -> bool {
        matches!(op, Operation::Update)
    }
    
    fn applies_to_schema(&self, _schema: &str) -> bool {
        true // Applies to all schemas
    }
}

#[async_trait]
impl DatabaseObserver for UpdateSqlExecutor {
    async fn execute(&self, ctx: &mut ObserverContext) -> Result<(), ObserverError> {
        if ctx.records.is_empty() {
            tracing::debug!("No records to process for UPDATE operation");
            return Ok(());
        }

        // Get database connection
        let pool = DatabaseManager::main_pool().await
            .map_err(|e| ObserverError::DatabaseError(e.to_string()))?;
        
        let mut results = Vec::new();
        let mut successful_operations = 0;
        
        // Process each StatefulRecord
        for record in &ctx.records {
            // Generate SQL operation from record state
            let sql_op = record.to_sql_operation(&ctx.schema_name)
                .map_err(|e| ObserverError::DatabaseError(e.to_string()))?;
            
            match self.execute_update_operation(&pool, sql_op).await {
                Ok(result) => {
                    results.push(result);
                    successful_operations += 1;
                }
                Err(error) => {
                    tracing::error!(
                        "UPDATE operation failed for record {:?}: {}",
                        record.id, error
                    );
                    ctx.errors.push(error);
                }
            }
        }
        
        tracing::info!(
            "UPDATE operations completed: {}/{} successful",
            successful_operations, ctx.records.len()
        );
        
        // Store results in context
        ctx.result = Some(results);
        
        Ok(())
    }
}

impl UpdateSqlExecutor {
    /// Execute UPDATE operation
    async fn execute_update_operation(&self, pool: &PgPool, sql_op: SqlOperation) -> Result<Value, ObserverError> {
        match sql_op {
            SqlOperation::Update { table, id, fields } => {
                if fields.is_empty() {
                    tracing::debug!("No changes for record {}, skipping update", id);
                    // Return a minimal record with just the ID
                    return Ok(serde_json::json!({ "id": id.to_string() }));
                }
                
                tracing::debug!("Updating record {} in {}: fields={:?}", id, table, fields.keys().collect::<Vec<_>>());
                
                // Build SET clause for only changed fields
                let set_clauses: Vec<String> = fields.keys()
                    .enumerate()
                    .map(|(i, field)| format!("\"{}\" = ${}", field, i + 1))
                    .collect();
                
                let values: Vec<Value> = fields.values().cloned().collect();
                
                let query = format!(
                    "UPDATE \"{}\" SET {}, updated_at = NOW() WHERE id = ${} RETURNING *",
                    table, set_clauses.join(", "), values.len() + 1
                );
                
                let mut q = sqlx::query(&query);
                for value in &values {
                    q = bind_param(q, value);
                }
                q = q.bind(id.to_string());
                
                let row = q.fetch_one(pool).await
                    .map_err(|e| ObserverError::DatabaseError(e.to_string()))?;
                
                self.row_to_json(row)
            }
            SqlOperation::NoOp => {
                tracing::debug!("No-op SQL operation for UPDATE");
                Ok(serde_json::json!({}))
            }
            _ => {
                Err(ObserverError::DatabaseError(
                    format!("UpdateSqlExecutor received unexpected operation: {:?}", sql_op)
                ))
            }
        }
    }
    
    /// Convert database row to JSON
    fn row_to_json(&self, row: sqlx::postgres::PgRow) -> Result<Value, ObserverError> {
        let mut record_data = Map::new();
        
        for (i, column) in row.columns().iter().enumerate() {
            let column_name = column.name();
            let value = self.extract_column_value(&row, i, column.type_info())?;
            record_data.insert(column_name.to_string(), value);
        }
        
        Ok(Value::Object(record_data))
    }
    
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