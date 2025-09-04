use serde_json::{json, Value};
use sqlx::{PgPool, Row, Column, TypeInfo};
use uuid::Uuid;
use std::collections::HashMap;

use crate::database::manager::DatabaseError;
use crate::database::record::{Record, RecordError, Operation};
use crate::filter::FilterData;
use crate::observer::{ObserverPipeline, register_all_sql_executors};

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

    pub async fn select_any(&self, filter_data: FilterData) -> Result<Vec<Record>, DatabaseError> {
        // Use pipeline's Record-aware method (handles all conversion internally)
        let pipeline = Self::create_pipeline();
        pipeline.select_any_records(self.table_name.clone(), filter_data).await
            .map_err(|e| DatabaseError::QueryError(e.to_string()))
    }

    pub async fn select_one(&self, filter_data: FilterData) -> Result<Option<Record>, DatabaseError> {
        let results = self.select_any(filter_data).await?;
        Ok(results.into_iter().next())
    }

    pub async fn select_404(&self, filter_data: FilterData) -> Result<Record, DatabaseError> {
        match self.select_one(filter_data).await? {
            Some(record) => Ok(record),
            None => Err(DatabaseError::NotFound("Record not found".to_string()))
        }
    }

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
        pipeline.create_all_records(self.table_name.clone(), records).await
            .map_err(|e| DatabaseError::QueryError(e.to_string()))
    }

    /// Create record from API JSON input
    pub async fn create_from_json(&self, json: Value) -> Result<Record, DatabaseError> {
        let record = Record::from_json(json)
            .map_err(|e| DatabaseError::QueryError(format!("Invalid input: {}", e)))?;
        self.create_one(record).await
    }

    // ========================================
    // UPDATE Operations  
    // ========================================

    /// Update a single record by ID
    pub async fn update_one(&self, id: Uuid, updates: HashMap<String, Value>) -> Result<Record, DatabaseError> {
        let results = self.update_all(vec![(id, updates)]).await?;
        Self::extract_single_result(results, "update_one")
    }

    /// Update multiple records by ID (the clean way!)
    pub async fn update_all(&self, updates: Vec<(Uuid, HashMap<String, Value>)>) -> Result<Vec<Record>, DatabaseError> {
        use crate::observer::Operation as PipelineOperation;
        
        if updates.is_empty() {
            return Ok(Vec::new());
        }
        
        // Load existing records
        let ids: Vec<Uuid> = updates.iter().map(|(id, _)| *id).collect();
        let existing_records = self.select_ids(ids).await?;
        
        // Apply changes to each record (this is SO much cleaner!)
        let mut updated_records = Vec::new();
        for (update_id, changes) in updates {
            if let Some(mut record) = existing_records.iter().find(|r| r.id() == Some(update_id)).cloned() {
                record.set_operation(Operation::Update);
                record.apply_changes(changes);
                updated_records.push(record);
            } else {
                return Err(DatabaseError::NotFound(format!("Record with ID {} not found", update_id)));
            }
        }
        
        // Use pipeline's Record-aware method (handles all conversion internally)
        let pipeline = Self::create_pipeline();
        pipeline.update_all_records(self.table_name.clone(), updated_records).await
            .map_err(|e| DatabaseError::QueryError(e.to_string()))
    }

    /// Update records matching filter criteria (now incredibly simple!)
    pub async fn update_any(&self, filter: FilterData, changes: HashMap<String, Value>) -> Result<Vec<Record>, DatabaseError> {
        if changes.is_empty() {
            return Ok(vec![]);
        }

        // 1. Find records matching filter
        let mut records = self.select_any(filter).await?;
        
        if records.is_empty() {
            return Ok(vec![]);
        }

        // 2. Apply changes to each record (so clean!)
        for record in &mut records {
            record.set_operation(Operation::Update);
            record.apply_changes(changes.clone());
        }

        // 3. Use pipeline's Record-aware method (handles all conversion internally)
        let pipeline = Self::create_pipeline();
        pipeline.update_all_records(self.table_name.clone(), records).await
            .map_err(|e| DatabaseError::QueryError(e.to_string()))
    }

    /// Update multiple records by ID array
    pub async fn update_ids(&self, ids: Vec<Uuid>, changes: HashMap<String, Value>) -> Result<Vec<Record>, DatabaseError> {
        if ids.is_empty() {
            return Ok(vec![]);
        }
        
        let updates = ids.into_iter()
            .map(|id| (id, changes.clone()))
            .collect();
        
        self.update_all(updates).await
    }

    // Additional methods can be implemented as needed using the same patterns:
    // - delete_one, delete_all, delete_any, delete_ids
    // - revert_one, revert_all, revert_any, revert_ids  
    // - update_404, delete_404, revert_404
    // - access_one, access_all, access_any, access_404
    //
    // All follow the same clean pattern:
    // 1. Load/create Records
    // 2. Apply operations using Record methods
    // 3. Run through observer pipeline
    // 4. Return Records
}

