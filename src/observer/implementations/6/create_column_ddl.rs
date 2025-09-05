// Ring 6: Create Column DDL Executor - handles ALTER TABLE ADD COLUMN after column record insert
use async_trait::async_trait;
use serde_json::{Value, Map};
use sqlx::PgPool;
use uuid::Uuid;

use crate::observer::traits::{Observer, Ring6, ObserverRing, Operation};
use crate::observer::context::ObserverContext;
use crate::observer::error::ObserverError;

/// Ring 6: Create Column DDL Executor - executes ALTER TABLE ADD COLUMN when column record is inserted
#[derive(Default)]
pub struct CreateColumnDdl;

impl Observer for CreateColumnDdl {
    fn name(&self) -> &'static str { 
        "CreateColumnDdl" 
    }
    
    fn ring(&self) -> ObserverRing { 
        ObserverRing::PostDatabase 
    }
    
    fn applies_to_operation(&self, op: Operation) -> bool {
        matches!(op, Operation::Create)
    }
    
    fn applies_to_schema(&self, schema: &str) -> bool {
        schema == "columns" // Only apply to operations on the columns table
    }
}

#[async_trait]
impl Ring6 for CreateColumnDdl {
    async fn execute(&self, context: &mut ObserverContext) -> Result<(), ObserverError> {
        // Get the newly inserted column record from context
        let records = &context.records;
        
        if records.is_empty() {
            return Ok(()); // No records to process
        }

        for record in records {
            // Extract column information from the inserted record
            let schema_name = record.get("schema_name")
                .and_then(|v| v.as_str())
                .ok_or_else(|| ObserverError::ValidationError("Schema name missing from column record".to_string()))?;
                
            let column_name = record.get("column_name")
                .and_then(|v| v.as_str())
                .ok_or_else(|| ObserverError::ValidationError("Column name missing from column record".to_string()))?;

            // Check if this column is being added to an existing table (not initial schema creation)
            if self.is_initial_schema_creation(context, schema_name).await? {
                // Skip DDL execution - table is being created by CreateSchemaDdl observer
                tracing::debug!("Skipping column DDL for '{}' - part of initial schema creation", column_name);
                continue;
            }

            // Get table name from schema
            let table_name = self.get_table_name_for_schema(context, schema_name).await?;
            
            // Generate ALTER TABLE ADD COLUMN DDL
            let ddl = self.generate_add_column_ddl(&table_name, record)?;
            
            // Execute DDL
            let pool = context.get_pool()
                .ok_or_else(|| ObserverError::ValidationError("Database pool not available".to_string()))?;
                
            sqlx::query(&ddl)
                .execute(pool)
                .await
                .map_err(|e| ObserverError::DatabaseError(format!("Failed to add column {} to table {}: {}", column_name, table_name, e)))?;
                
            tracing::info!("Added column '{}' to table '{}' for schema '{}'", column_name, table_name, schema_name);
        }

        Ok(())
    }
}

impl CreateColumnDdl {
    async fn is_initial_schema_creation(&self, context: &ObserverContext, schema_name: &str) -> Result<bool, ObserverError> {
        // Check if this is part of initial schema creation by looking at context metadata
        // If we're in the middle of creating a schema, skip individual column DDL
        if let Some(metadata) = context.get_metadata::<serde_json::Map<String, serde_json::Value>>() {
            if let Some(creating_schema) = metadata.get("creating_schema") {
                if let Some(current_schema) = creating_schema.as_str() {
                    return Ok(current_schema == schema_name);
                }
            }
        }
        Ok(false)
    }
    
    async fn get_table_name_for_schema(&self, context: &ObserverContext, schema_name: &str) -> Result<String, ObserverError> {
        // Get database pool directly (same pattern as Ring 5 observers)
        let pool = crate::database::manager::DatabaseManager::main_pool().await
            .map_err(|e| ObserverError::DatabaseError(e.to_string()))?;
            
        let row = sqlx::query("SELECT table_name FROM schemas WHERE name = $1 AND deleted_at IS NULL")
            .bind(schema_name)
            .fetch_one(&pool)
            .await
            .map_err(|e| ObserverError::DatabaseError(format!("Failed to get table name for schema {}: {}", schema_name, e)))?;
            
        let table_name: String = row.get("table_name");
        Ok(table_name)
    }
    
    fn generate_add_column_ddl(&self, table_name: &str, column_record: &Map<String, Value>) -> Result<String, ObserverError> {
        let column_name = column_record.get("column_name")
            .and_then(|v| v.as_str())
            .ok_or_else(|| ObserverError::ValidationError("Column name missing".to_string()))?;
            
        let pg_type = column_record.get("pg_type")
            .and_then(|v| v.as_str())
            .ok_or_else(|| ObserverError::ValidationError("PostgreSQL type missing".to_string()))?;
            
        let is_required = column_record.get("is_required")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);
            
        let nullable = if is_required { " NOT NULL" } else { "" };
        
        let default_value = if let Some(default) = column_record.get("default_value") {
            if let Some(default_str) = default.as_str() {
                format!(" DEFAULT '{}'", default_str.replace('\'', "''"))
            } else {
                String::new()
            }
        } else {
            String::new()
        };
        
        let ddl = format!(
            "ALTER TABLE \"{}\" ADD COLUMN \"{}\" {}{}{}",
            table_name, column_name, pg_type, nullable, default_value
        );
        
        Ok(ddl)
    }
}