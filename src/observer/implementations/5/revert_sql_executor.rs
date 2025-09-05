// Ring 5: Revert SQL Executor - handles REVERT operations (undo soft delete)
use async_trait::async_trait;
use serde_json::{Value, Map};
use sqlx::{PgPool, Row, Column, TypeInfo};
use uuid::Uuid;

use crate::observer::traits::{Observer, Ring5, ObserverRing, Operation};
use crate::observer::context::ObserverContext;
use crate::observer::error::ObserverError;
use crate::database::manager::DatabaseManager;

/// Ring 5: Revert SQL Executor - handles REVERT operations only
#[derive(Default)]
pub struct RevertSqlExecutor;

impl Observer for RevertSqlExecutor {
    fn name(&self) -> &'static str { 
        "RevertSqlExecutor" 
    }
    
    fn ring(&self) -> ObserverRing { 
        ObserverRing::Database 
    }
    
    fn applies_to_operation(&self, op: Operation) -> bool {
        matches!(op, Operation::Revert)
    }
    
    fn applies_to_schema(&self, _schema: &str) -> bool {
        true // Applies to all schemas
    }
}

#[async_trait]
impl Ring5 for RevertSqlExecutor {
    async fn execute(&self, ctx: &mut ObserverContext) -> Result<(), ObserverError> {
        if ctx.records.is_empty() {
            tracing::debug!("No records to process for REVERT operation");
            return Ok(());
        }

        // Get database connection
        let pool = DatabaseManager::main_pool().await
            .map_err(|e| ObserverError::DatabaseError(e.to_string()))?;
        
        let mut results = Vec::new();
        let mut successful_operations = 0;
        
        // Process each Record
        for record in &ctx.records {
            match self.execute_revert_record(&pool, record, &ctx.schema_name).await {
                Ok(result) => {
                    results.push(result);
                    successful_operations += 1;
                }
                Err(error) => {
                    tracing::error!(
                        "REVERT operation failed for record {:?}: {}",
                        record.id(), error
                    );
                    ctx.errors.push(error);
                }
            }
        }
        
        tracing::info!(
            "REVERT operations completed: {}/{} successful",
            successful_operations, ctx.records.len()
        );
        
        // Store results in context
        ctx.result = Some(results);
        
        Ok(())
    }
}

impl RevertSqlExecutor {
    /// Execute REVERT operation for a Record (undo soft delete)
    async fn execute_revert_record(
        &self, 
        pool: &PgPool, 
        record: &crate::database::record::Record, 
        table_name: &str
    ) -> Result<Value, ObserverError> {
        let record_id = record.id().ok_or_else(|| {
            ObserverError::DatabaseError("REVERT operation requires record ID".to_string())
        })?;
        
        tracing::debug!("Reverting soft-deleted record {} in {}", record_id, table_name);
        
        let query = format!(
            "UPDATE \"{}\" SET trashed_at = NULL, updated_at = NOW() WHERE id = $1 RETURNING *",
            table_name
        );
        
        let row = sqlx::query(&query)
            .bind(record_id.to_string())
            .fetch_one(pool)
            .await
            .map_err(|e| ObserverError::DatabaseError(e.to_string()))?;
        
        self.row_to_json(row)
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