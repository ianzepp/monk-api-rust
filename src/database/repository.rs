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
    T: for<'r> FromRow<'r, PgRow> + Send + Unpin + Serialize,
{
    pub fn new(table_name: impl Into<String>, pool: PgPool) -> Self {
        Self {
            table_name: table_name.into(),
            pool,
            _phantom: std::marker::PhantomData,
        }
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

    /// Create a single record from StatefulRecord
    pub async fn create_one(&self, record: crate::observer::stateful_record::StatefulRecord) -> Result<T, DatabaseError> {
        let results = self.create_all(vec![record]).await?;
        results.into_iter().next()
            .ok_or_else(|| DatabaseError::QueryError("create_one produced no results".to_string()))
    }

    /// Create multiple records from StatefulRecord array  
    pub async fn create_all(&self, records: Vec<crate::observer::stateful_record::StatefulRecord>) -> Result<Vec<T>, DatabaseError> {
        // TODO: Implement observer pipeline integration
        // This should:
        // 1. Run pre-create observers on StatefulRecord instances
        // 2. Generate SQL operations from StatefulRecord.to_sql_operation()
        // 3. Execute SQL in transaction
        // 4. Run post-create observers
        // 5. Return created records as T instances
        
        unimplemented!("create_all requires observer pipeline integration")
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
        // TODO: Implement observer pipeline integration
        // This should:
        // 1. Fetch existing records by IDs 
        // 2. Create StatefulRecord instances with changes
        // 3. Run pre-update observers
        // 4. Generate SQL operations from StatefulRecord.to_sql_operation()
        // 5. Execute SQL in transaction
        // 6. Run post-update observers
        // 7. Return updated records as T instances
        
        unimplemented!("update_all requires observer pipeline integration")
    }

    /// Update records matching filter criteria
    pub async fn update_any(&self, filter: FilterData, changes: std::collections::HashMap<String, serde_json::Value>) -> Result<Vec<T>, DatabaseError> {
        // 1. Find all records matching the filter
        let records = self.select_any(filter).await?;
        
        if records.is_empty() {
            return Ok(vec![]);
        }

        // TODO: Extract IDs from T instances and delegate to update_all
        // This requires T to have an ID field we can access
        unimplemented!("update_any requires ID extraction from generic type T")
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

