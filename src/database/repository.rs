use serde_json::{json, Value};
use sqlx::{PgPool, Row, Column, TypeInfo};
use uuid::Uuid;
use std::collections::HashMap;

use crate::database::manager::DatabaseError;
use crate::database::record::Record;
use crate::types::Operation;
use crate::filter::FilterData;
use crate::observer::{ObserverPipeline, register_all_sql_executors};

/// Query parameter that can be either a UUID or a FilterData
#[derive(Debug, Clone)]
pub enum QueryParam {
    Id(Uuid),
    Filter(FilterData),
}

impl From<Uuid> for QueryParam {
    fn from(id: Uuid) -> Self {
        QueryParam::Id(id)
    }
}

impl From<FilterData> for QueryParam {
    fn from(filter: FilterData) -> Self {
        QueryParam::Filter(filter)
    }
}

impl QueryParam {
    /// Convert to FilterData for internal use
    pub fn to_filter_data(self) -> FilterData {
        match self {
            QueryParam::Id(id) => FilterData {
                where_clause: Some(serde_json::json!({ "id": id })),
                ..Default::default()
            },
            QueryParam::Filter(filter) => filter,
        }
    }
}

pub struct Repository {
    table_name: String,
    pool: PgPool,
}

impl Repository {
    pub fn new(table_name: impl Into<String>, pool: PgPool) -> Self {
        Self {
            table_name: table_name.into(),
            pool,
        }
    }

    /// Create an observer pipeline with all SQL executors registered
    /// REST API requires all CRUD operations to be available
    fn create_pipeline() -> ObserverPipeline {
        let mut pipeline = ObserverPipeline::new();
        register_all_sql_executors(&mut pipeline);
        pipeline
    }


    /// Helper to extract single result from bulk operation
    fn extract_single_result(results: Vec<Record>, operation_name: &str) -> Result<Record, DatabaseError> {
        results.into_iter().next()
            .ok_or_else(|| DatabaseError::NotFound(format!("{} produced no results", operation_name)))
    }

    /// Execute raw SQL and convert results to Records
    async fn execute_sql(&self, query: &str, params: &[Value]) -> Result<Vec<Record>, DatabaseError> {
        let mut sql_query = sqlx::query(query);
        for param in params {
            sql_query = self.bind_param(sql_query, param);
        }

        let rows = sql_query.fetch_all(&self.pool).await
            .map_err(DatabaseError::Sqlx)?;

        let mut records = Vec::new();
        for row in rows {
            let record = self.row_to_record(row)?;
            records.push(record);
        }

        Ok(records)
    }

    /// Execute DDL (Data Definition Language) statements like CREATE TABLE, ALTER TABLE, DROP TABLE
    pub async fn execute_ddl(&self, ddl: &str) -> Result<(), DatabaseError> {
        sqlx::query(ddl)
            .execute(&self.pool)
            .await
            .map_err(DatabaseError::Sqlx)?;
        Ok(())
    }

    /// Convert database row to Record
    fn row_to_record(&self, row: sqlx::postgres::PgRow) -> Result<Record, DatabaseError> {
        let mut data = HashMap::new();

        for (i, column) in row.columns().iter().enumerate() {
            let column_name = column.name();
            let value = self.extract_column_value(&row, i, column.type_info())?;
            data.insert(column_name.to_string(), value);
        }

        Ok(Record::from_sql_data(data))
    }

    /// Extract typed value from database column
    fn extract_column_value(
        &self,
        row: &sqlx::postgres::PgRow,
        index: usize,
        type_info: &sqlx::postgres::PgTypeInfo,
    ) -> Result<Value, DatabaseError> {
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

    /// Bind parameter to SQL query
    fn bind_param<'q>(
        &self,
        q: sqlx::query::Query<'q, sqlx::Postgres, sqlx::postgres::PgArguments>,
        v: &'q Value,
    ) -> sqlx::query::Query<'q, sqlx::Postgres, sqlx::postgres::PgArguments> {
        match v {
            Value::Null => {
                let none: Option<String> = None;
                q.bind(none)
            }
            Value::Bool(b) => q.bind(*b),
            Value::Number(n) => {
                if let Some(i) = n.as_i64() {
                    q.bind(i)
                } else if let Some(u) = n.as_u64() {
                    q.bind(u as i64)
                } else if let Some(f) = n.as_f64() {
                    q.bind(f)
                } else {
                    q.bind(n.to_string())
                }
            }
            Value::String(s) => q.bind(s),
            Value::Array(_arr) => {
                // Arrays should be expanded before binding; for now pass through as JSON
                q.bind(v)
            }
            Value::Object(_) => q.bind(v), // JSONB
        }
    }

    /// Select all records with optional pagination
    pub async fn select_all(&self, limit: Option<i32>, offset: Option<i32>) -> Result<Vec<Record>, DatabaseError> {
        let filter_data = FilterData {
            select: None,
            where_clause: None,
            order: None,
            limit,
            offset,
        };
        self.select_any(filter_data).await
    }

    /// Select records with filter criteria
    pub async fn select_any(&self, filter_data: FilterData) -> Result<Vec<Record>, DatabaseError> {
        // Use pipeline's Record-aware method (handles all conversion internally)
        let pipeline = Self::create_pipeline();
        pipeline.select(&self.table_name, filter_data, self.pool.clone()).await
            .map_err(|e| DatabaseError::QueryError(e.to_string()))
    }

    /// Select single record - accepts either UUID or FilterData
    pub async fn select_one(&self, query: impl Into<QueryParam>) -> Result<Option<Record>, DatabaseError> {
        let filter_data = query.into().to_filter_data();
        let results = self.select_any(filter_data).await?;
        Ok(results.into_iter().next())
    }

    /// Select record or return 404 - accepts either UUID or FilterData
    pub async fn select_404(&self, query: impl Into<QueryParam>) -> Result<Record, DatabaseError> {
        match self.select_one(query).await? {
            Some(record) => Ok(record),
            None => Err(DatabaseError::NotFound("Record not found".to_string()))
        }
    }

    // REMOVED: update_by_id_404() - use update_404(uuid, record) instead
    // The unified update_404() method now handles Uuid inputs seamlessly

    pub async fn count(&self, filter_data: FilterData) -> Result<i64, DatabaseError> {
        use crate::filter::Filter;

        // Use Filter system to build count query
        let mut filter = Filter::new(&self.table_name)
            .map_err(|e| DatabaseError::QueryError(e.to_string()))?;

        filter.assign(filter_data)
            .map_err(|e| DatabaseError::QueryError(e.to_string()))?;

        // Build count query instead of select
        let query = format!("SELECT COUNT(*) FROM \"{}\"", self.table_name);
        let row = sqlx::query(&query)
            .fetch_one(&self.pool)
            .await
            .map_err(DatabaseError::Sqlx)?;

        let count: i64 = row.try_get(0).map_err(DatabaseError::Sqlx)?;
        Ok(count)
    }

    pub async fn select_ids(&self, ids: Vec<Uuid>) -> Result<Vec<Record>, DatabaseError> {
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

    /// Create a single record
    pub async fn create_one(&self, record: Record) -> Result<Record, DatabaseError> {
        let results = self.create_all(vec![record]).await?;
        Self::extract_single_result(results, "create_one")
    }

    /// Create multiple records
    pub async fn create_all(&self, mut records: Vec<Record>) -> Result<Vec<Record>, DatabaseError> {
        // Set operation type for all records
        for record in &mut records {
            record.set_operation(Operation::Create);
        }

        // Use pipeline's Record-aware method (handles all conversion internally)
        let pipeline = Self::create_pipeline();
        pipeline.modify(crate::types::Operation::Create, &self.table_name, records, self.pool.clone()).await
            .map_err(|e| DatabaseError::QueryError(e.to_string()))
    }

    // ========================================
    // UPSERT Operations
    // ========================================

    /// Upsert a single record (update if ID exists, create if no ID)
    pub async fn upsert_one(&self, record: Record) -> Result<Record, DatabaseError> {
        let results = self.upsert_all(vec![record]).await?;
        Self::extract_single_result(results, "upsert_one")
    }

    /// Upsert multiple records (update if ID exists, create if no ID)
    pub async fn upsert_all(&self, records: Vec<Record>) -> Result<Vec<Record>, DatabaseError> {
        if records.is_empty() {
            return Ok(Vec::new());
        }

        // Split records into updates (with ID) and creates (without ID)
        let mut records_to_update = Vec::new();
        let mut records_to_create = Vec::new();

        for mut record in records {
            if record.id().is_some() {
                record.set_operation(Operation::Update);
                records_to_update.push(record);
            } else {
                record.set_operation(Operation::Create);
                records_to_create.push(record);
            }
        }

        let mut all_results = Vec::new();

        // Update existing records
        if !records_to_update.is_empty() {
            let updated_records = self.update_all(records_to_update).await?;
            all_results.extend(updated_records);
        }

        // Create new records
        if !records_to_create.is_empty() {
            let created_records = self.create_all(records_to_create).await?;
            all_results.extend(created_records);
        }

        Ok(all_results)
    }

    // ========================================
    // UPDATE Operations
    // ========================================

    // REMOVED: update_one(id, HashMap) - use update_one(Record) instead
    // REMOVED: update_all(Vec<(Uuid, HashMap)>) - use update_all(Vec<Record>) instead

    /// Update a single record
    pub async fn update_one(&self, record: Record) -> Result<Record, DatabaseError> {
        let results = self.update_all(vec![record]).await?;
        Self::extract_single_result(results, "update_one")
    }

    /// Update multiple records
    pub async fn update_all(&self, mut records: Vec<Record>) -> Result<Vec<Record>, DatabaseError> {
        if records.is_empty() {
            return Ok(Vec::new());
        }

        // Validate IDs and set operation type for all records
        for (index, record) in records.iter_mut().enumerate() {
            if record.id().is_none() {
                return Err(DatabaseError::InvalidOperation(
                    format!("UPDATE requires all records to have IDs. Record at index {} is missing an ID", index)
                ));
            }
            record.set_operation(Operation::Update);
        }

        // Use pipeline's Record-aware method (handles all conversion internally)
        let pipeline = Self::create_pipeline();
        pipeline.modify(crate::types::Operation::Update, &self.table_name, records, self.pool.clone()).await
            .map_err(|e| DatabaseError::QueryError(e.to_string()))
    }

    // REMOVED: update_any(filter, HashMap) - API layer should build Records with changes
    // Use select_any() + Record.apply_changes() + update_all() pattern instead

    // REMOVED: update_ids(Vec<Uuid>, HashMap) - API layer should build Records with IDs and changes
    // Use select_ids() + Record.apply_changes() + update_all() pattern instead

    // ========================================
    // DELETE Operations
    // ========================================

    /// Delete a single record
    pub async fn delete_one(&self, record: Record) -> Result<Record, DatabaseError> {
        let results = self.delete_all(vec![record]).await?;
        Self::extract_single_result(results, "delete_one")
    }

    /// Delete multiple records
    pub async fn delete_all(&self, mut records: Vec<Record>) -> Result<Vec<Record>, DatabaseError> {
        if records.is_empty() {
            return Ok(Vec::new());
        }

        // Validate IDs and set operation type for all records
        for (index, record) in records.iter_mut().enumerate() {
            if record.id().is_none() {
                return Err(DatabaseError::InvalidOperation(
                    format!("DELETE requires all records to have IDs. Record at index {} is missing an ID", index)
                ));
            }
            record.set_operation(Operation::Delete);
        }

        // Use pipeline's Record-aware method (handles all conversion internally)
        let pipeline = Self::create_pipeline();
        pipeline.modify(crate::types::Operation::Delete, &self.table_name, records, self.pool.clone()).await
            .map_err(|e| DatabaseError::QueryError(e.to_string()))
    }

    /// Delete record or return 404 - accepts either UUID or FilterData
    pub async fn delete_404(&self, query: impl Into<QueryParam>) -> Result<Record, DatabaseError> {
        let mut record = self.select_404(query).await?;  // 404 if not found
        record.mark_deleted();  // Mark as soft deleted
        self.delete_one(record).await
    }

    // ========================================
    // UPDATE with 404 Operations
    // ========================================

    /// Update record or return 404 - accepts either UUID or FilterData  
    pub async fn update_404(&self, query: impl Into<QueryParam>, updates: Record) -> Result<Record, DatabaseError> {
        let mut existing_record = self.select_404(query).await?;  // 404 if not found
        
        // Apply updates to existing record
        let update_data = updates.to_hashmap();
        existing_record.apply_changes(update_data);
        existing_record.set_operation(Operation::Update);
        
        self.update_one(existing_record).await
    }

    // ========================================
    // Additional Utility Methods
    // ========================================

    /// Delete multiple records by IDs
    pub async fn delete_ids(&self, ids: Vec<Uuid>) -> Result<Vec<Record>, DatabaseError> {
        if ids.is_empty() {
            return Ok(vec![]);
        }
        
        let records = self.select_ids(ids).await?;
        self.delete_all(records).await
    }

    /// Update records matching filter criteria with the provided changes
    pub async fn update_any(&self, filter_data: FilterData, change: Record) -> Result<Vec<Record>, DatabaseError> {
        let matching_records = self.select_any(filter_data).await?;
        
        if matching_records.is_empty() {
            return Ok(Vec::new());
        }
        
        // Apply changes to all matching records
        let change_data = change.to_hashmap();
        let mut updated_records = Vec::new();
        for mut record in matching_records {
            record.apply_changes(change_data.clone());
            updated_records.push(record);
        }
        
        self.update_all(updated_records).await
    }

    /// Delete records matching filter criteria
    pub async fn delete_any(&self, filter_data: FilterData) -> Result<Vec<Record>, DatabaseError> {
        let records = self.select_any(filter_data).await?;
        self.delete_all(records).await
    }

}
