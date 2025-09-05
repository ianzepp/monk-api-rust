// Ring 6: Update Schema DDL Executor - handles table alterations after schema record update
use async_trait::async_trait;
use serde_json::{Value, Map};
use sqlx::PgPool;
use uuid::Uuid;

use crate::observer::traits::{Observer, Ring6, ObserverRing, Operation};
use crate::observer::context::ObserverContext;
use crate::observer::error::ObserverError;

/// Ring 6: Update Schema DDL Executor - handles table changes when schema record is updated
#[derive(Default)]
pub struct UpdateSchemaDdl;

impl Observer for UpdateSchemaDdl {
    fn name(&self) -> &'static str { 
        "UpdateSchemaDdl" 
    }
    
    fn ring(&self) -> ObserverRing { 
        ObserverRing::PostDatabase 
    }
    
    fn applies_to_operation(&self, op: Operation) -> bool {
        matches!(op, Operation::Update)
    }
    
    fn applies_to_schema(&self, schema: &str) -> bool {
        schema == "schemas" // Only apply to operations on the schemas table
    }
}

#[async_trait]
impl Ring6 for UpdateSchemaDdl {
    async fn execute(&self, context: &mut ObserverContext) -> Result<(), ObserverError> {
        // Get the updated schema record from context
        let records = &context.records
            .ok_or_else(|| ObserverError::ValidationError("No records in context".to_string()))?;

        for record in records {
            // Skip if this is a deletion (handled by DeleteSchemaDdl)
            let was_deleted = record.get("trashed_at").and_then(|v| v.as_str()).is_some() ||
                             record.get("deleted_at").and_then(|v| v.as_str()).is_some();
                             
            if was_deleted {
                continue;
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
                tracing::warn!("Attempted to update protected schema '{}', skipping DDL", schema_name);
                continue;
            }

            // For schema updates, we primarily rely on individual column operations
            // Schema-level updates are typically metadata changes (status, description, etc.)
            // The actual table structure changes happen via column record updates
            
            tracing::info!("Schema '{}' metadata updated (table: '{}')", schema_name, table_name);
            
            // Note: Major schema restructuring (like renaming tables) would require
            // more complex DDL operations and migration planning, which should be
            // handled through explicit migration processes rather than automatic observers
        }

        Ok(())
    }
}

impl UpdateSchemaDdl {
    fn is_protected_schema(&self, schema_name: &str) -> bool {
        ["schemas", "users", "columns"].contains(&schema_name)
    }
}