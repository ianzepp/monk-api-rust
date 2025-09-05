// Ring 6: Delete Schema DDL Executor - handles DROP TABLE after schema record delete
use async_trait::async_trait;
use serde_json::{Value, Map};
use sqlx::PgPool;
use uuid::Uuid;

use crate::observer::traits::{Observer, Ring6, ObserverRing, Operation};
use crate::observer::context::ObserverContext;
use crate::observer::error::ObserverError;

/// Ring 6: Delete Schema DDL Executor - executes DROP TABLE when schema record is deleted/trashed
#[derive(Default)]
pub struct DeleteSchemaDdl;

impl Observer for DeleteSchemaDdl {
    fn name(&self) -> &'static str { 
        "DeleteSchemaDdl" 
    }
    
    fn ring(&self) -> ObserverRing { 
        ObserverRing::PostDatabase 
    }
    
    fn applies_to_operation(&self, op: Operation) -> bool {
        matches!(op, Operation::Update | Operation::Delete)
    }
    
    fn applies_to_schema(&self, schema: &str) -> bool {
        schema == "schemas" // Only apply to operations on the schemas table
    }
}

#[async_trait]
impl Ring6 for DeleteSchemaDdl {
    async fn execute(&self, context: &mut ObserverContext) -> Result<(), ObserverError> {
        // Get the updated/deleted schema record from context
        let records = context.get_records()
            .ok_or_else(|| ObserverError::ValidationError("No records in context".to_string()))?;

        for record in records {
            // Check if this record was soft deleted (trashed_at or deleted_at set)
            let was_deleted = record.get("trashed_at").and_then(|v| v.as_str()).is_some() ||
                             record.get("deleted_at").and_then(|v| v.as_str()).is_some();
                             
            if !was_deleted {
                continue; // Not a deletion, skip
            }

            // Extract schema information from the record
            let schema_name = record.get("name")
                .and_then(|v| v.as_str())
                .ok_or_else(|| ObserverError::ValidationError("Schema name missing from record".to_string()))?;
                
            let table_name = record.get("table_name")
                .and_then(|v| v.as_str())
                .unwrap_or(schema_name);

            // Check if schema is protected
            if self.is_protected_schema(schema_name) {
                tracing::warn!("Attempted to delete protected schema '{}', skipping table drop", schema_name);
                continue;
            }

            // Generate DROP TABLE DDL
            let ddl = format!("DROP TABLE IF EXISTS \"{}\"", table_name);
            
            // Execute DDL
            let pool = context.get_pool()
                .ok_or_else(|| ObserverError::ValidationError("Database pool not available".to_string()))?;
                
            sqlx::query(&ddl)
                .execute(pool)
                .await
                .map_err(|e| ObserverError::DatabaseError(format!("Failed to drop table {}: {}", table_name, e)))?;
                
            tracing::info!("Dropped table '{}' for deleted schema '{}'", table_name, schema_name);
            
            // Also clean up related column records
            self.cleanup_column_records(context, schema_name).await?;
        }

        Ok(())
    }
}

impl DeleteSchemaDdl {
    fn is_protected_schema(&self, schema_name: &str) -> bool {
        ["schemas", "users", "columns"].contains(&schema_name)
    }
    
    async fn cleanup_column_records(&self, context: &ObserverContext, schema_name: &str) -> Result<(), ObserverError> {
        let pool = context.get_pool()
            .ok_or_else(|| ObserverError::ValidationError("Database pool not available".to_string()))?;
            
        // Soft delete all column records for this schema
        let now = chrono::Utc::now().to_rfc3339();
        sqlx::query(
            "UPDATE columns SET deleted_at = $1, updated_at = $1 WHERE schema_name = $2 AND deleted_at IS NULL"
        )
        .bind(&now)
        .bind(schema_name)
        .execute(pool)
        .await
        .map_err(|e| ObserverError::DatabaseError(format!("Failed to cleanup column records for schema {}: {}", schema_name, e)))?;
        
        tracing::info!("Cleaned up column records for deleted schema '{}'", schema_name);
        Ok(())
    }
}