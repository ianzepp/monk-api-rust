use serde_json::json;
use serde::Serialize;
use sqlx::{self, postgres::PgRow, FromRow, PgPool};
use uuid::Uuid;

use crate::database::manager::DatabaseError;
use crate::database::query_builder::QueryBuilder;
use crate::filter::FilterData;

pub struct Repository<T> {
    table_name: String,
    pool: PgPool,
    _phantom: std::marker::PhantomData<T>,
}

impl<T> Repository<T>
where
    T: for<'r> FromRow<'r, PgRow> + Send + Unpin + Serialize + serde::de::DeserializeOwned,
{
    pub fn new(table_name: impl Into<String>, pool: PgPool) -> Self {
        Self {
            table_name: table_name.into(),
            pool,
            _phantom: std::marker::PhantomData,
        }
    }

    /// Create an observer pipeline with all SQL executors registered
    /// REST API requires all CRUD operations to be available
    fn create_pipeline() -> crate::observer::ObserverPipeline {
        use crate::observer::{ObserverPipeline, register_all_sql_executors};
        
        let mut pipeline = ObserverPipeline::new();
        register_all_sql_executors(&mut pipeline);
        pipeline
    }

    pub async fn select_any(&self, filter_data: FilterData) -> Result<Vec<T>, DatabaseError> {
        QueryBuilder::<T>::new(&self.table_name)?
            .filter(filter_data)?
            .select_all(&self.pool)
            .await
    }

    pub async fn select_one(&self, filter_data: FilterData) -> Result<Option<T>, DatabaseError> {
        QueryBuilder::<T>::new(&self.table_name)?
            .filter(filter_data)?
            .select_optional(&self.pool)
            .await
    }

    pub async fn select_404(&self, filter_data: FilterData) -> Result<T, DatabaseError> {
        match QueryBuilder::<T>::new(&self.table_name)?
            .filter(filter_data)?
            .select_one(&self.pool)
            .await
        {
            Ok(row) => Ok(row),
            Err(DatabaseError::Sqlx(sqlx::Error::RowNotFound)) => {
                Err(DatabaseError::NotFound("Record not found".to_string()))
            }
            Err(other) => Err(other),
        }
    }

    pub async fn count(&self, filter_data: FilterData) -> Result<i64, DatabaseError> {
        QueryBuilder::<T>::new(&self.table_name)?
            .filter(filter_data)?
            .count(&self.pool)
            .await
    }

    pub async fn select_ids(&self, ids: Vec<Uuid>) -> Result<Vec<T>, DatabaseError> {
        if ids.is_empty() {
            return Ok(vec![]);
        }
        let filter = FilterData {
            where_clause: Some(json!({ "id": { "$in": ids } })),
            ..Default::default()
        };
        self.select_any(filter).await
    }

    // ========================================
    // CREATE Operations
    // ========================================

    /// Create a single record from typed record
    pub async fn create_one(&self, record: T) -> Result<T, DatabaseError> {
        let results = self.create_all(vec![record]).await?;
        results.into_iter().next()
            .ok_or_else(|| DatabaseError::QueryError("create_one produced no results".to_string()))
    }

    /// Create multiple records from typed record array  
    pub async fn create_all(&self, records: Vec<T>) -> Result<Vec<T>, DatabaseError> {
        use crate::observer::Operation;
        use crate::observer::stateful_record::{StatefulRecord, RecordOperation};
        
        // Convert T instances to StatefulRecord for pipeline processing
        let mut stateful_records = Vec::new();
        for record in records {
            // Serialize T to JSON for StatefulRecord
            let record_json = serde_json::to_value(record)
                .map_err(|e| DatabaseError::QueryError(format!("Failed to serialize record for pipeline: {}", e)))?;
            
            if let serde_json::Value::Object(record_map) = record_json {
                let stateful_record = StatefulRecord::new(record_map, RecordOperation::Create);
                stateful_records.push(stateful_record);
            } else {
                return Err(DatabaseError::QueryError("Record must serialize to JSON object".to_string()));
            }
        }
        
        // Create observer pipeline with all SQL executors (REST API requirement)
        let pipeline = Self::create_pipeline();
        
        // Execute through observer pipeline
        let observer_result = pipeline.execute_crud(
            Operation::Create,
            self.table_name.clone(),
            stateful_records,
        ).await?;
        
        if !observer_result.success {
            return Err(DatabaseError::QueryError(
                format!("Observer pipeline validation failed: {} errors", observer_result.errors.len())
            ));
        }
        
        // Convert results back to typed records
        let results = observer_result.result.unwrap_or_default();
        let typed_results: Result<Vec<T>, _> = results.into_iter()
            .map(|value| serde_json::from_value(value))
            .collect();
        
        typed_results.map_err(|e| DatabaseError::QueryError(
            format!("Failed to deserialize results: {}", e)
        ))
    }

    // ========================================
    // UPDATE Operations  
    // ========================================

    /// Update a single record by ID
    pub async fn update_one(&self, id: Uuid, updates: std::collections::HashMap<String, serde_json::Value>) -> Result<T, DatabaseError> {
        let results = self.update_all(vec![(id, updates)]).await?;
        results.into_iter().next()
            .ok_or_else(|| DatabaseError::NotFound("Record not found for update".to_string()))
    }

    /// Update multiple records by ID
    pub async fn update_all(&self, updates: Vec<(Uuid, std::collections::HashMap<String, serde_json::Value>)>) -> Result<Vec<T>, DatabaseError> {
        use crate::observer::Operation;
        use crate::observer::stateful_record::{StatefulRecord, RecordOperation};
        
        if updates.is_empty() {
            return Ok(Vec::new());
        }
        
        // First, fetch existing records for update operations
        let ids: Vec<Uuid> = updates.iter().map(|(id, _)| *id).collect();
        let existing_records = self.select_ids(ids).await?;
        
        // Convert to StatefulRecord instances with updates
        let mut stateful_records = Vec::new();
        for (update_id, changes) in updates {
            // Find existing record
            if let Some(existing) = existing_records.iter()
                .find(|record| {
                    // Assuming T can be serialized to get ID field
                    if let Ok(value) = serde_json::to_value(record) {
                        if let Some(id_str) = value.get("id").and_then(|v| v.as_str()) {
                            if let Ok(record_id) = Uuid::parse_str(id_str) {
                                return record_id == update_id;
                            }
                        }
                    }
                    false
                }) {
                
                // Convert existing record to JSON
                let existing_json = serde_json::to_value(existing)
                    .map_err(|e| DatabaseError::QueryError(format!("Failed to serialize existing record: {}", e)))?;
                
                if let serde_json::Value::Object(existing_map) = existing_json {
                    // Convert changes HashMap to serde_json::Map  
                    let changes_map: serde_json::Map<String, serde_json::Value> = changes.into_iter().collect();
                    
                    let stateful_record = StatefulRecord::existing(
                        existing_map,
                        Some(changes_map),
                        RecordOperation::Update
                    );
                    
                    stateful_records.push(stateful_record);
                }
            } else {
                return Err(DatabaseError::NotFound(format!("Record with ID {} not found", update_id)));
            }
        }
        
        // Create observer pipeline with all SQL executors (REST API requirement)
        let pipeline = Self::create_pipeline();
        
        // Execute through observer pipeline
        let observer_result = pipeline.execute_crud(
            Operation::Update,
            self.table_name.clone(),
            stateful_records,
        ).await?;
        
        if !observer_result.success {
            return Err(DatabaseError::QueryError(
                format!("Observer pipeline validation failed: {} errors", observer_result.errors.len())
            ));
        }
        
        // Convert results back to typed records
        let results = observer_result.result.unwrap_or_default();
        let typed_results: Result<Vec<T>, _> = results.into_iter()
            .map(|value| serde_json::from_value(value))
            .collect();
        
        typed_results.map_err(|e| DatabaseError::QueryError(
            format!("Failed to deserialize results: {}", e)
        ))
    }

    /// Update records matching filter criteria
    pub async fn update_any(&self, filter: FilterData, changes: std::collections::HashMap<String, serde_json::Value>) -> Result<Vec<T>, DatabaseError> {
        if changes.is_empty() {
            return Ok(vec![]);
        }

        // Create observer pipeline with all SQL executors (REST API requirement)
        let pipeline = Self::create_pipeline();

        // 1. SELECT at pipeline level to get StatefulRecords (no conversion to T)
        let select_result = pipeline.select_any(self.table_name.clone(), filter).await
            .map_err(|e| DatabaseError::QueryError(format!("Pipeline SELECT failed: {}", e)))?;

        if !select_result.success {
            return Err(DatabaseError::QueryError(
                format!("SELECT pipeline validation failed: {} errors", select_result.errors.len())
            ));
        }

        if select_result.records.is_empty() {
            return Ok(vec![]);
        }

        // 2. Apply changes to existing StatefulRecords in-memory (no database queries)
        let changes_map: serde_json::Map<String, serde_json::Value> = changes.into_iter().collect();
        let updated_records: Vec<crate::observer::stateful_record::StatefulRecord> = select_result.records
            .into_iter()
            .map(|record| {
                // Create new StatefulRecord with existing data + changes
                use crate::observer::stateful_record::{StatefulRecord, RecordOperation};
                
                // Get original data from StatefulRecord
                let original_data = record.original.unwrap_or(record.modified);
                
                // Create updated StatefulRecord with changes
                StatefulRecord::existing(
                    original_data,
                    Some(changes_map.clone()),
                    RecordOperation::Update
                )
            })
            .collect();

        // 3. UPDATE at pipeline level with pre-loaded StatefulRecords (skips Ring 0 data loading)
        let update_result = pipeline.update_all(self.table_name.clone(), updated_records).await
            .map_err(|e| DatabaseError::QueryError(format!("Pipeline UPDATE failed: {}", e)))?;

        if !update_result.success {
            return Err(DatabaseError::QueryError(
                format!("UPDATE pipeline validation failed: {} errors", update_result.errors.len())
            ));
        }

        // 4. Convert StatefulRecord results back to T
        let results: Vec<serde_json::Value> = update_result.records
            .into_iter()
            .map(|record| serde_json::Value::Object(record.modified))
            .collect();

        let typed_results: Result<Vec<T>, _> = results.into_iter()
            .map(|value| serde_json::from_value(value))
            .collect();

        typed_results.map_err(|e| DatabaseError::QueryError(
            format!("Failed to deserialize results: {}", e)
        ))
    }

    /// Update multiple records by ID array
    pub async fn update_ids(&self, ids: Vec<Uuid>, changes: std::collections::HashMap<String, serde_json::Value>) -> Result<Vec<T>, DatabaseError> {
        if ids.is_empty() {
            return Ok(vec![]);
        }
        
        let updates = ids.into_iter()
            .map(|id| (id, changes.clone()))
            .collect();
        
        self.update_all(updates).await
    }

    // ========================================
    // DELETE Operations (Soft Delete)
    // ========================================

    /// Soft delete a single record by ID
    pub async fn delete_one(&self, id: Uuid) -> Result<T, DatabaseError> {
        let results = self.delete_ids(vec![id]).await?;
        results.into_iter().next()
            .ok_or_else(|| DatabaseError::NotFound("Record not found for deletion".to_string()))
    }

    /// Soft delete multiple records by creating StatefulRecord instances
    pub async fn delete_all(&self, deletes: Vec<std::collections::HashMap<String, serde_json::Value>>) -> Result<Vec<T>, DatabaseError> {
        // TODO: Implement observer pipeline integration
        // This should:
        // 1. Create StatefulRecord instances for delete operation
        // 2. Run pre-delete observers
        // 3. Generate SQL operations (SET trashed_at = NOW())
        // 4. Execute SQL in transaction
        // 5. Run post-delete observers
        // 6. Return soft-deleted records as T instances
        
        unimplemented!("delete_all requires observer pipeline integration")
    }

    /// Soft delete records matching filter criteria
    pub async fn delete_any(&self, filter: FilterData) -> Result<Vec<T>, DatabaseError> {
        // 1. Find all records matching the filter
        let records = self.select_any(filter).await?;
        
        if records.is_empty() {
            return Ok(vec![]);
        }

        // TODO: Convert T instances to delete records and delegate to delete_all
        unimplemented!("delete_any requires conversion from generic type T")
    }

    /// Soft delete multiple records by ID array
    pub async fn delete_ids(&self, ids: Vec<Uuid>) -> Result<Vec<T>, DatabaseError> {
        if ids.is_empty() {
            return Ok(vec![]);
        }
        
        // Convert IDs to delete records with just ID field
        let delete_records: Vec<std::collections::HashMap<String, serde_json::Value>> = ids.into_iter()
            .map(|id| {
                let mut record = std::collections::HashMap::new();
                record.insert("id".to_string(), serde_json::Value::String(id.to_string()));
                record
            })
            .collect();
            
        self.delete_all(delete_records).await
    }

    // ========================================
    // REVERT Operations (Undo Soft Delete)
    // ========================================

    /// Revert a single soft-deleted record by ID
    pub async fn revert_one(&self, id: Uuid) -> Result<T, DatabaseError> {
        let results = self.revert_ids(vec![id]).await?;
        results.into_iter().next()
            .ok_or_else(|| DatabaseError::NotFound("Record not found for revert".to_string()))
    }

    /// Revert multiple soft-deleted records
    pub async fn revert_all(&self, reverts: Vec<std::collections::HashMap<String, serde_json::Value>>) -> Result<Vec<T>, DatabaseError> {
        // TODO: Implement observer pipeline integration
        // This should:
        // 1. Create StatefulRecord instances for revert operation
        // 2. Run pre-revert observers
        // 3. Generate SQL operations (SET trashed_at = NULL)
        // 4. Execute SQL in transaction
        // 5. Run post-revert observers
        // 6. Return reverted records as T instances
        
        unimplemented!("revert_all requires observer pipeline integration")
    }

    /// Revert soft-deleted records matching filter criteria
    pub async fn revert_any(&self, filter: FilterData) -> Result<Vec<T>, DatabaseError> {
        // Note: This requires include_trashed=true to find trashed records
        // TODO: Implement filter with trashed records included
        unimplemented!("revert_any requires trashed record filtering")
    }

    /// Revert multiple soft-deleted records by ID array
    pub async fn revert_ids(&self, ids: Vec<Uuid>) -> Result<Vec<T>, DatabaseError> {
        if ids.is_empty() {
            return Ok(vec![]);
        }
        
        // Convert IDs to revert records with ID and trashed_at = null
        let revert_records: Vec<std::collections::HashMap<String, serde_json::Value>> = ids.into_iter()
            .map(|id| {
                let mut record = std::collections::HashMap::new();
                record.insert("id".to_string(), serde_json::Value::String(id.to_string()));
                record.insert("trashed_at".to_string(), serde_json::Value::Null);
                record
            })
            .collect();
            
        self.revert_all(revert_records).await
    }

    // ========================================
    // 404 Operations (Throw error if not found)
    // ========================================

    /// Update record with 404 error if not found
    pub async fn update_404(&self, filter: FilterData, changes: std::collections::HashMap<String, serde_json::Value>) -> Result<T, DatabaseError> {
        // First ensure record exists (throws if not found)
        let record = self.select_404(filter.clone()).await?;
        
        // TODO: Extract ID from record and call update_one
        // This requires T to have accessible ID field
        unimplemented!("update_404 requires ID extraction from generic type T")
    }

    /// Delete record with 404 error if not found  
    pub async fn delete_404(&self, filter: FilterData) -> Result<T, DatabaseError> {
        // First ensure record exists (throws if not found)
        let record = self.select_404(filter).await?;
        
        // TODO: Extract ID from record and call delete_one
        // This requires T to have accessible ID field
        unimplemented!("delete_404 requires ID extraction from generic type T")
    }

    /// Revert record with 404 error if not found
    pub async fn revert_404(&self, filter: FilterData) -> Result<T, DatabaseError> {
        // First ensure record exists (throws if not found)
        let record = self.select_404(filter).await?;
        
        // TODO: Extract ID from record and call revert_one
        // This requires T to have accessible ID field
        unimplemented!("revert_404 requires ID extraction from generic type T")
    }

    // ========================================
    // ACCESS Control Operations
    // ========================================

    /// Update access permissions for a single record
    pub async fn access_one(&self, id: Uuid, access_changes: std::collections::HashMap<String, serde_json::Value>) -> Result<T, DatabaseError> {
        // TODO: Implement access control updates
        // This should:
        // 1. Verify record exists
        // 2. Filter to only allow access_* fields
        // 3. Execute UPDATE for access fields only
        // 4. Return updated record
        
        unimplemented!("access_one requires access control implementation")
    }

    /// Update access permissions for multiple records
    pub async fn access_all(&self, updates: Vec<(Uuid, std::collections::HashMap<String, serde_json::Value>)>) -> Result<Vec<T>, DatabaseError> {
        // TODO: Batch access control updates
        unimplemented!("access_all requires access control implementation")
    }

    /// Update access permissions for records matching filter
    pub async fn access_any(&self, filter: FilterData, access_changes: std::collections::HashMap<String, serde_json::Value>) -> Result<Vec<T>, DatabaseError> {
        // TODO: Filter-based access control updates
        unimplemented!("access_any requires access control implementation")
    }

    /// Update access permissions with 404 error if not found
    pub async fn access_404(&self, filter: FilterData, access_changes: std::collections::HashMap<String, serde_json::Value>) -> Result<T, DatabaseError> {
        // TODO: Access control with 404 error handling
        unimplemented!("access_404 requires access control implementation")
    }
}

