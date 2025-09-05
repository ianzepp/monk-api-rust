// Ring 0: Data Preparation - loads existing data and merges updates
use async_trait::async_trait;
use uuid::Uuid;
use std::collections::HashMap;

use crate::observer::traits::{Observer, Ring0, ObserverRing, Operation};
use crate::observer::context::ObserverContext;
use crate::observer::error::ObserverError;
use crate::database::repository::Repository;
use crate::filter::FilterData;
use crate::database::record::Record;

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
impl Ring0 for DataPreparationObserver {
    async fn execute(&self, ctx: &mut ObserverContext) -> Result<(), ObserverError> {
        // Only process operations that need existing data
        match ctx.operation {
            Operation::Update | Operation::Delete | Operation::Revert => {
                self.prepare_data_with_existing(ctx).await
            }
            _ => {
                tracing::debug!("No data preparation needed for operation: {:?}", ctx.operation);
                Ok(())
            }
        }
    }
}

impl DataPreparationObserver {
    /// Unified data preparation for all operations that need existing data
    async fn prepare_data_with_existing(&self, ctx: &mut ObserverContext) -> Result<(), ObserverError> {
        if ctx.records.is_empty() {
            tracing::debug!("No records to prepare data for");
            return Ok(());
        }

        tracing::info!("Preparing data for {} records in schema: {}", ctx.records.len(), ctx.schema_name);

        // Extract all IDs from records
        let ids: Vec<Uuid> = ctx.records.iter()
            .filter_map(|record| record.id())
            .collect();

        if ids.is_empty() {
            return Err(ObserverError::ValidationError(
                format!("{:?} operations require record IDs", ctx.operation)
            ));
        }

        // Create repository and query existing records
        let repository = self.create_repository(&ctx.schema_name).await?;
        let existing_records = match ctx.operation {
            Operation::Revert => {
                // Query for trashed records only
                let filter_data = FilterData {
                    where_clause: Some(serde_json::json!({ 
                        "id": { "$in": ids },
                        "trashed_at": { "$ne": null }
                    })),
                    ..Default::default()
                };
                repository.select_any(filter_data).await
                    .map_err(|e| ObserverError::DatabaseError(e.to_string()))?
            }
            _ => {
                // Query for normal records (UPDATE, DELETE)
                repository.select_ids(ids).await
                    .map_err(|e| ObserverError::DatabaseError(e.to_string()))?
            }
        };

        // Process records based on operation
        match ctx.operation {
            Operation::Update => {
                // For UPDATE: inject existing data into current records for change tracking
                let existing_by_id: HashMap<Uuid, Record> = existing_records.into_iter()
                    .filter_map(|r| r.id().map(|id| (id, r)))
                    .collect();

                let mut successful_preparations = 0;
                for record in &mut ctx.records {
                    if let Some(record_id) = record.id() {
                        if let Some(existing_record) = existing_by_id.get(&record_id) {
                            // Skip if record already has original data
                            if record.original().is_none() {
                                record.inject(existing_record.to_hashmap());
                                successful_preparations += 1;
                            }
                        } else {
                            ctx.errors.push(ObserverError::ValidationError(
                                format!("Record {} not found for update", record_id)
                            ));
                        }
                    }
                }

                tracing::info!("UPDATE data preparation: {}/{} records prepared", 
                    successful_preparations, ctx.records.len());
            }
            Operation::Delete | Operation::Revert => {
                // For DELETE/REVERT: replace context records with existing records
                let operation = ctx.operation; // Capture before moving
                ctx.records = existing_records.into_iter()
                    .map(|mut record| {
                        record.set_operation(operation);
                        record
                    })
                    .collect();

                tracing::info!("{:?} data preparation: {} records prepared", 
                    operation, ctx.records.len());
            }
            _ => unreachable!() // Already filtered in execute()
        }

        Ok(())
    }

    /// Create a Repository for the schema
    async fn create_repository(&self, schema_name: &str) -> Result<Repository, ObserverError> {
        use crate::database::manager::DatabaseManager;
        
        let pool = DatabaseManager::main_pool().await
            .map_err(|e| ObserverError::DatabaseError(e.to_string()))?;
        
        Ok(Repository::new(schema_name, pool))
    }
}