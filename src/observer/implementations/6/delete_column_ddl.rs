// Ring 6: Delete Column DDL Executor - handles ALTER TABLE DROP COLUMN after column record delete
use async_trait::async_trait;
use serde_json::{Value, Map};
use sqlx::{PgPool, Row};
use uuid::Uuid;

use crate::observer::traits::{Observer, Ring6, ObserverRing, Operation};
use crate::observer::context::ObserverContext;
use crate::observer::error::ObserverError;

/// Ring 6: Delete Column DDL Executor - executes ALTER TABLE DROP COLUMN when column record is deleted/trashed
#[derive(Default)]
pub struct DeleteColumnDdl;

impl Observer for DeleteColumnDdl {
    fn name(&self) -> &'static str { 
        "DeleteColumnDdl" 
    }
    
    fn ring(&self) -> ObserverRing { 
        ObserverRing::PostDatabase 
    }
    
    fn applies_to_operation(&self, op: Operation) -> bool {
        matches!(op, Operation::Update | Operation::Delete)
    }
    
    fn applies_to_schema(&self, schema: &str) -> bool {
        schema == "columns" // Only apply to operations on the columns table
    }
}

#[async_trait]
impl Ring6 for DeleteColumnDdl {
    async fn execute(&self, context: &mut ObserverContext) -> Result<(), ObserverError> {
        // Get the updated/deleted column record from context
        let records = &context.records;
        
        if records.is_empty() {
            return Ok(()); // No records to process
        }

        for record in records {
            // Check if this record was soft deleted (trashed_at or deleted_at set)
            let was_deleted = record.get("trashed_at").and_then(|v| v.as_str()).is_some() ||
                             record.get("deleted_at").and_then(|v| v.as_str()).is_some();
                             
            if !was_deleted {
                continue; // Not a deletion, skip
            }

            // Extract column information from the record
            let schema_name = record.get("schema_name")
                .and_then(|v| v.as_str())
                .ok_or_else(|| ObserverError::ValidationError("Schema name missing from column record".to_string()))?;
                
            let column_name = record.get("column_name")
                .and_then(|v| v.as_str())
                .ok_or_else(|| ObserverError::ValidationError("Column name missing from column record".to_string()))?;

            // Check if the parent schema still exists and is not deleted
            if !self.schema_exists_and_active(context, schema_name).await? {
                tracing::debug!("Parent schema '{}' is deleted, skipping column drop for '{}'", schema_name, column_name);
                continue;
            }

            // Get table name from schema
            let table_name = self.get_table_name_for_schema(context, schema_name).await?;
            
            // Generate ALTER TABLE DROP COLUMN DDL
            let ddl = format!("ALTER TABLE \"{}\" DROP COLUMN IF EXISTS \"{}\"", table_name, column_name);
            
            // Execute DDL
            let pool = context.get_pool();
                
            sqlx::query(&ddl)
                .execute(pool)
                .await
                .map_err(|e| ObserverError::DatabaseError(format!("Failed to drop column {} from table {}: {}", column_name, table_name, e)))?;
                
            tracing::info!("Dropped column '{}' from table '{}' for schema '{}'", column_name, table_name, schema_name);
        }

        Ok(())
    }
}

impl DeleteColumnDdl {
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
}