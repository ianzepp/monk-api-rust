// Ring 6: Update Column DDL Executor - handles ALTER COLUMN after column record update
use async_trait::async_trait;
use serde_json::{Value, Map};
use sqlx::Row;

use crate::observer::traits::{Observer, Ring6, ObserverRing, Operation};
use crate::observer::context::ObserverContext;
use crate::observer::error::ObserverError;

/// Ring 6: Update Column DDL Executor - handles ALTER COLUMN when column record is updated
#[derive(Default)]
pub struct UpdateColumnDdl;

impl Observer for UpdateColumnDdl {
    fn name(&self) -> &'static str { 
        "UpdateColumnDdl" 
    }
    
    fn ring(&self) -> ObserverRing { 
        ObserverRing::PostDatabase 
    }
    
    fn applies_to_operation(&self, op: Operation) -> bool {
        matches!(op, Operation::Update)
    }
    
    fn applies_to_schema(&self, schema: &str) -> bool {
        schema == "columns" // Only apply to operations on the columns table
    }
}

#[async_trait]
impl Ring6 for UpdateColumnDdl {
    async fn execute(&self, context: &mut ObserverContext) -> Result<(), ObserverError> {
        // Get the updated column record from context
        let records = &context.records;
        
        if records.is_empty() {
            return Ok(()); // No records to process
        }

        for record in records {
            // Skip if this column was deleted (handled by DeleteColumnDdl)
            let was_deleted = record.get("trashed_at").and_then(|v| v.as_str()).is_some() ||
                             record.get("deleted_at").and_then(|v| v.as_str()).is_some();
                             
            if was_deleted {
                continue;
            }

            // Extract column information from the record
            let schema_name = record.get("schema_name")
                .and_then(|v| v.as_str())
                .ok_or_else(|| ObserverError::ValidationError("Schema name missing from column record".to_string()))?;
                
            let column_name = record.get("column_name")
                .and_then(|v| v.as_str())
                .ok_or_else(|| ObserverError::ValidationError("Column name missing from column record".to_string()))?;

            // Check if the parent schema exists and is active
            if !self.schema_exists_and_active(context, schema_name).await? {
                tracing::debug!("Parent schema '{}' is inactive, skipping column update for '{}'", schema_name, column_name);
                continue;
            }

            // Get table name from schema
            let table_name = self.get_table_name_for_schema(context, schema_name).await?;
            
            // Column updates are complex because they can involve:
            // 1. Type changes (may require data migration)
            // 2. Constraint changes (NOT NULL, DEFAULT values)
            // 3. Name changes (RENAME COLUMN)
            
            // For safety, we'll focus on safe changes for now:
            // - Adding/removing DEFAULT values
            // - Changing column comments/descriptions
            
            let ddl_operations = self.generate_safe_column_updates(&table_name, &record.to_map())?;
            
            if ddl_operations.is_empty() {
                tracing::debug!("No safe DDL operations for column '{}' update", column_name);
                continue;
            }

            // Execute DDL operations
            let pool = context.get_pool();
                
            for ddl in ddl_operations {
                sqlx::query(&ddl)
                    .execute(pool)
                    .await
                    .map_err(|e| ObserverError::DatabaseError(format!("Failed to update column {} in table {}: {}", column_name, table_name, e)))?;
            }
                
            tracing::info!("Updated column '{}' in table '{}' for schema '{}'", column_name, table_name, schema_name);
        }

        Ok(())
    }
}

impl UpdateColumnDdl {
    async fn schema_exists_and_active(&self, context: &ObserverContext, schema_name: &str) -> Result<bool, ObserverError> {
        let pool = context.get_pool();
            
        let result = sqlx::query(
            "SELECT COUNT(*) as count FROM schemas WHERE name = $1 AND deleted_at IS NULL AND trashed_at IS NULL"
        )
        .bind(schema_name)
        .fetch_one(pool)
        .await
        .map_err(|e| ObserverError::DatabaseError(format!("Failed to check schema existence: {}", e)))?;
        
        let count: i64 = result.get("count");
        Ok(count > 0)
    }
    
    async fn get_table_name_for_schema(&self, context: &ObserverContext, schema_name: &str) -> Result<String, ObserverError> {
        let pool = context.get_pool();
            
        let row = sqlx::query("SELECT table_name FROM schemas WHERE name = $1 AND deleted_at IS NULL")
            .bind(schema_name)
            .fetch_one(pool)
            .await
            .map_err(|e| ObserverError::DatabaseError(format!("Failed to get table name for schema {}: {}", schema_name, e)))?;
            
        let table_name: String = row.get("table_name");
        Ok(table_name)
    }
    
    fn generate_safe_column_updates(&self, table_name: &str, column_record: &Map<String, Value>) -> Result<Vec<String>, ObserverError> {
        let mut ddl_operations = Vec::new();
        
        let column_name = column_record.get("column_name")
            .and_then(|v| v.as_str())
            .ok_or_else(|| ObserverError::ValidationError("Column name missing".to_string()))?;
        
        // Safe operation 1: Update DEFAULT value
        if let Some(default_value) = column_record.get("default_value") {
            if let Some(default_str) = default_value.as_str() {
                if !default_str.is_empty() {
                    ddl_operations.push(format!(
                        "ALTER TABLE \"{}\" ALTER COLUMN \"{}\" SET DEFAULT '{}'",
                        table_name, column_name, default_str.replace('\'', "''")
                    ));
                } else {
                    // Remove default if empty string
                    ddl_operations.push(format!(
                        "ALTER TABLE \"{}\" ALTER COLUMN \"{}\" DROP DEFAULT",
                        table_name, column_name
                    ));
                }
            }
        }
        
        // Safe operation 2: Update column comment (if PostgreSQL supports it via COMMENT ON)
        if let Some(description) = column_record.get("description") {
            if let Some(desc_str) = description.as_str() {
                ddl_operations.push(format!(
                    "COMMENT ON COLUMN \"{}\".\"{}\" IS '{}'",
                    table_name, column_name, desc_str.replace('\'', "''")
                ));
            }
        }
        
        // Note: More complex changes like type alterations, NOT NULL constraints, etc.
        // should be handled through explicit migration processes rather than automatic updates
        // to avoid data loss or constraint violations
        
        Ok(ddl_operations)
    }
}