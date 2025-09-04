// Ring 0: Data Preparation - loads existing data and merges updates
use async_trait::async_trait;
use serde_json::{Value, Map};
use uuid::Uuid;
use std::collections::HashMap;

use crate::observer::traits::{Observer, GenericObserver, ObserverRing, Operation};
use crate::observer::context::ObserverContext;
use crate::observer::error::ObserverError;
use crate::database::repository::Repository;
use crate::filter::FilterData;

/// Ring 0: Data Preparation Observer - loads existing data and merges updates
#[derive(Default)]
pub struct DataPreparationObserver;

impl Observer for DataPreparationObserver {
    fn name(&self) -> &'static str { 
        "DataPreparationObserver" 
    }
    
    fn ring(&self) -> ObserverRing { 
        ObserverRing::DataPreparation 
    }
    
    fn applies_to_operation(&self, op: Operation) -> bool {
        // Only applies to operations that need existing data
        matches!(op, Operation::Update | Operation::Delete | Operation::Revert)
    }
    
    fn applies_to_schema(&self, _schema: &str) -> bool {
        true // Applies to all schemas
    }
}

#[async_trait]
impl GenericObserver for DataPreparationObserver {
    async fn execute(&self, ctx: &mut ObserverContext) -> Result<(), ObserverError> {
        if ctx.records.is_empty() {
            tracing::debug!("No records to prepare data for");
            return Ok(());
        }

        tracing::info!("Preparing data for {} records in schema: {}", ctx.records.len(), ctx.schema_name);

        match ctx.operation {
            Operation::Update => self.prepare_update_data(ctx).await,
            Operation::Delete => self.prepare_delete_data(ctx).await,
            Operation::Revert => self.prepare_revert_data(ctx).await,
            _ => {
                tracing::debug!("No data preparation needed for operation: {:?}", ctx.operation);
                Ok(())
            }
        }
    }
}

impl DataPreparationObserver {
    /// Prepare data for UPDATE operations - load existing records and merge changes
    async fn prepare_update_data(&self, ctx: &mut ObserverContext) -> Result<(), ObserverError> {
        // Skip data loading if records already have original data loaded
        let needs_preparation = ctx.records.iter().any(|record| record.original().is_none());
        if !needs_preparation {
            tracing::debug!("UPDATE records already have original data loaded, skipping data preparation");
            return Ok(());
        }

        // Extract all IDs from Records that need data loading
        let ids: Vec<Uuid> = ctx.records.iter()
            .filter_map(|record| record.id())
            .collect();

        if ids.is_empty() {
            return Err(ObserverError::ValidationError("UPDATE operations require record IDs".to_string()));
        }

        // Use Repository to bulk load existing records
        let repository = self.create_repository(&ctx.schema_name).await?;
        let existing_records = repository.select_ids(ids).await
            .map_err(|e| ObserverError::DatabaseError(e.to_string()))?;

        // Convert existing records to lookup map by ID
        let mut existing_by_id: HashMap<Uuid, crate::database::record::Record> = HashMap::new();
        for record in existing_records {
            if let Some(record_id) = record.id() {
                existing_by_id.insert(record_id, record);
            }
        }

        // Merge existing data with changes for each Record
        let mut successful_preparations = 0;

        for record in &mut ctx.records {
            if let Some(record_id) = record.id() {
                if let Some(existing_record) = existing_by_id.get(&record_id) {
                    // Inject existing data into the record for change tracking
                    record.inject(existing_record.to_hashmap());
                    successful_preparations += 1;
                } else {
                    let error = ObserverError::NotFound(format!("Record {} not found for update", record_id));
                    tracing::error!("Failed to find existing record for update: {}", error);
                    ctx.errors.push(error);
                }
            } else {
                let error = ObserverError::ValidationError("Record missing ID for update".to_string());
                tracing::error!("UPDATE record missing ID: {}", error);
                ctx.errors.push(error);
            }
        }

        tracing::info!(
            "UPDATE data preparation completed: {}/{} successful",
            successful_preparations, ctx.records.len()
        );

        Ok(())
    }

    /// Prepare data for DELETE operations - load existing records for soft delete
    async fn prepare_delete_data(&self, ctx: &mut ObserverContext) -> Result<(), ObserverError> {
        // Skip data loading if records already have original data loaded
        let needs_preparation = ctx.records.iter().any(|record| record.original.is_none());
        if !needs_preparation {
            tracing::debug!("DELETE records already have original data loaded, skipping data preparation");
            return Ok(());
        }

        // Extract all IDs from StatefulRecords that need data loading
        let ids: Vec<Uuid> = ctx.records.iter()
            .filter_map(|record| record.id)
            .collect();

        if ids.is_empty() {
            return Err(ObserverError::ValidationError("DELETE operations require record IDs".to_string()));
        }

        // Use Repository to bulk load existing records (excluding trashed)
        let repository = self.create_repository(&ctx.schema_name).await?;
        let existing_records = repository.select_ids(ids).await
            .map_err(|e| ObserverError::DatabaseError(e.to_string()))?;

        // Convert existing records to StatefulRecords for deletion
        let mut prepared_records = Vec::new();
        let mut successful_preparations = 0;

        for record in existing_records {
            // Serialize to JSON to get data map
            let record_json = serde_json::to_value(record)
                .map_err(|e| ObserverError::DatabaseError(format!("Failed to serialize existing record: {}", e)))?;
            
            if let Value::Object(record_map) = record_json {
                // Create StatefulRecord with existing data for deletion
                let prepared_record = StatefulRecord::existing(
                    record_map,
                    None, // No changes for delete - just mark for deletion
                    RecordOperation::Delete
                );
                prepared_records.push(prepared_record);
                successful_preparations += 1;
            }
        }

        // Replace records with prepared versions
        ctx.records = prepared_records;

        tracing::info!(
            "DELETE data preparation completed: {}/{} successful",
            successful_preparations, ctx.records.len()
        );

        Ok(())
    }

    /// Prepare data for REVERT operations - load existing soft-deleted records
    async fn prepare_revert_data(&self, ctx: &mut ObserverContext) -> Result<(), ObserverError> {
        // Skip data loading if records already have original data loaded
        let needs_preparation = ctx.records.iter().any(|record| record.original.is_none());
        if !needs_preparation {
            tracing::debug!("REVERT records already have original data loaded, skipping data preparation");
            return Ok(());
        }

        // Extract all IDs from StatefulRecords that need data loading
        let ids: Vec<Uuid> = ctx.records.iter()
            .filter_map(|record| record.id)
            .collect();

        if ids.is_empty() {
            return Err(ObserverError::ValidationError("REVERT operations require record IDs".to_string()));
        }

        // Use Repository to load trashed records with filter
        let repository = self.create_repository(&ctx.schema_name).await?;
        let filter_data = FilterData {
            where_clause: Some(serde_json::json!({ 
                "id": { "$in": ids },
                "trashed_at": { "$ne": null } // Only trashed records
            })),
            include_trashed: Some(true), // Include trashed records
            ..Default::default()
        };

        let existing_records = repository.select_any(filter_data).await
            .map_err(|e| ObserverError::DatabaseError(e.to_string()))?;

        // Convert existing trashed records to StatefulRecords for revert
        let mut prepared_records = Vec::new();
        let mut successful_preparations = 0;

        for record in existing_records {
            // Serialize to JSON to get data map
            let record_json = serde_json::to_value(record)
                .map_err(|e| ObserverError::DatabaseError(format!("Failed to serialize existing record: {}", e)))?;
            
            if let Value::Object(record_map) = record_json {
                // Create StatefulRecord with existing data for revert
                let prepared_record = StatefulRecord::existing(
                    record_map,
                    None, // No changes for revert - just unmark deletion
                    RecordOperation::Revert
                );
                prepared_records.push(prepared_record);
                successful_preparations += 1;
            }
        }

        // Replace records with prepared versions
        ctx.records = prepared_records;

        tracing::info!(
            "REVERT data preparation completed: {}/{} successful",
            successful_preparations, ctx.records.len()
        );

        Ok(())
    }

    /// Create a generic Repository for the schema to leverage existing bulk query methods
    async fn create_repository(&self, schema_name: &str) -> Result<Repository<serde_json::Value>, ObserverError> {
        use crate::database::manager::DatabaseManager;
        
        let pool = DatabaseManager::main_pool().await
            .map_err(|e| ObserverError::DatabaseError(e.to_string()))?;
        
        Ok(Repository::new(schema_name, pool))
    }
}