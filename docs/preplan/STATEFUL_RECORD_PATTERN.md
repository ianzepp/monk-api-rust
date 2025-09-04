# Stateful Record Pattern - Advanced Observer System

This document outlines a superior **Stateful Record Pattern** where records carry both original and modified state through the observer pipeline, enabling precise change tracking, efficient database operations, and granular validation.

## Core Concept

Instead of passing raw JSON data through observers, we pass **StatefulRecord** objects that:
- Know their **original state** (from database)
- Track **modified state** (from API changes + observer modifications)
- Can **diff themselves** to identify exact changes
- Enable **precise validation** of specific field changes
- Support **efficient database operations** (only update changed fields)

## Architecture Overview

```rust
// Before: Raw JSON data through pipeline
Vec<serde_json::Value> -> Observer -> Observer -> Database

// After: Stateful records with change tracking
Vec<StatefulRecord> -> Observer -> Observer -> Database
                  ↓
              Knows original state, modified state, and diffs
```

## Core Types

### 1. StatefulRecord - The Heart of the Pattern

```rust
// src/observer/stateful_record.rs
use serde_json::{Value, Map};
use std::collections::HashMap;
use uuid::Uuid;
use chrono::{DateTime, Utc};

/// A record that tracks both original and modified state through the observer pipeline
#[derive(Debug, Clone)]
pub struct StatefulRecord {
    /// Unique identifier for this record
    pub id: Option<Uuid>,
    
    /// Original state from database (None for CREATE operations)
    pub original: Option<Map<String, Value>>,
    
    /// Current modified state (starts as API input data, modified by observers)
    pub modified: Map<String, Value>,
    
    /// Operation type for this record
    pub operation: RecordOperation,
    
    /// Change tracking metadata
    pub metadata: RecordMetadata,
}

/// Operation type for individual records
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum RecordOperation {
    Create,          // New record (original = None)
    Update,          // Existing record modification
    Delete,          // Soft delete operation
    Revert,          // Restore soft-deleted record
    NoChange,        // Record loaded but no changes needed
}

/// Metadata about record changes and processing
#[derive(Debug, Clone, Default)]
pub struct RecordMetadata {
    /// Fields modified by API request
    pub api_changes: Vec<String>,
    
    /// Fields modified by observers (with observer name)
    pub observer_changes: HashMap<String, String>,
    
    /// Validation results per field
    pub field_validations: HashMap<String, FieldValidationResult>,
    
    /// Security check results
    pub security_checks: Vec<SecurityCheck>,
    
    /// Timestamp when record entered pipeline
    pub pipeline_start: DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub struct FieldValidationResult {
    pub field: String,
    pub validator: String,
    pub result: ValidationResult,
    pub message: Option<String>,
}

#[derive(Debug, Clone)]
pub enum ValidationResult {
    Valid,
    Invalid(String),
    Warning(String),
}

#[derive(Debug, Clone)]
pub struct SecurityCheck {
    pub check_type: String,
    pub passed: bool,
    pub reason: Option<String>,
}
```

### 2. StatefulRecord Implementation

```rust
impl StatefulRecord {
    /// Create new record for CREATE operation
    pub fn create(data: Map<String, Value>) -> Self {
        let api_changes = data.keys().cloned().collect();
        
        Self {
            id: None,
            original: None,
            modified: data,
            operation: RecordOperation::Create,
            metadata: RecordMetadata {
                api_changes,
                pipeline_start: Utc::now(),
                ..Default::default()
            },
        }
    }
    
    /// Create existing record for UPDATE/DELETE operation
    pub fn existing(
        original: Map<String, Value>, 
        changes: Option<Map<String, Value>>,
        operation: RecordOperation
    ) -> Self {
        let id = original.get("id")
            .and_then(|v| v.as_str())
            .and_then(|s| Uuid::parse_str(s).ok());
        
        let mut modified = original.clone();
        let mut api_changes = Vec::new();
        
        // Apply API changes to create modified state
        if let Some(changes) = changes {
            api_changes = changes.keys().cloned().collect();
            for (key, value) in changes {
                modified.insert(key, value);
            }
        }
        
        Self {
            id,
            original: Some(original),
            modified,
            operation,
            metadata: RecordMetadata {
                api_changes,
                pipeline_start: Utc::now(),
                ..Default::default()
            },
        }
    }
    
    /// Get the current value of a field
    pub fn get_field(&self, field: &str) -> Option<&Value> {
        self.modified.get(field)
    }
    
    /// Set field value (called by observers to modify records)
    pub fn set_field(&mut self, field: &str, value: Value, observer_name: &str) {
        let old_value = self.modified.get(field).cloned();
        self.modified.insert(field.to_string(), value.clone());
        
        // Track that this observer modified this field
        self.metadata.observer_changes.insert(
            field.to_string(),
            observer_name.to_string()
        );
        
        tracing::debug!(
            "Observer {} modified field '{}': {:?} -> {:?}",
            observer_name, field, old_value, value
        );
    }
    
    /// Remove field (soft delete specific field)
    pub fn remove_field(&mut self, field: &str, observer_name: &str) {
        self.modified.remove(field);
        self.metadata.observer_changes.insert(
            field.to_string(),
            format!("{} (removed)", observer_name)
        );
    }
    
    /// Calculate diff between original and modified state
    pub fn calculate_changes(&self) -> RecordChanges {
        match &self.original {
            Some(original) => {
                let mut added = HashMap::new();
                let mut modified_fields = HashMap::new();
                let mut removed = Vec::new();
                
                // Find added and modified fields
                for (key, new_value) in &self.modified {
                    match original.get(key) {
                        Some(old_value) if old_value != new_value => {
                            modified_fields.insert(
                                key.clone(),
                                FieldChange {
                                    field: key.clone(),
                                    old_value: Some(old_value.clone()),
                                    new_value: Some(new_value.clone()),
                                    change_type: ChangeType::Modified,
                                }
                            );
                        }
                        None => {
                            added.insert(
                                key.clone(),
                                FieldChange {
                                    field: key.clone(),
                                    old_value: None,
                                    new_value: Some(new_value.clone()),
                                    change_type: ChangeType::Added,
                                }
                            );
                        }
                        _ => {} // No change
                    }
                }
                
                // Find removed fields
                for key in original.keys() {
                    if !self.modified.contains_key(key) {
                        removed.push(FieldChange {
                            field: key.clone(),
                            old_value: Some(original[key].clone()),
                            new_value: None,
                            change_type: ChangeType::Removed,
                        });
                    }
                }
                
                RecordChanges {
                    has_changes: !added.is_empty() || !modified_fields.is_empty() || !removed.is_empty(),
                    added,
                    modified: modified_fields,
                    removed,
                }
            }
            None => {
                // CREATE operation - all fields are "added"
                let added = self.modified.iter()
                    .map(|(key, value)| {
                        (key.clone(), FieldChange {
                            field: key.clone(),
                            old_value: None,
                            new_value: Some(value.clone()),
                            change_type: ChangeType::Added,
                        })
                    })
                    .collect();
                
                RecordChanges {
                    has_changes: !added.is_empty(),
                    added,
                    modified: HashMap::new(),
                    removed: Vec::new(),
                }
            }
        }
    }
    
    /// Check if specific field was changed
    pub fn field_changed(&self, field: &str) -> bool {
        match &self.original {
            Some(original) => {
                let old_value = original.get(field);
                let new_value = self.modified.get(field);
                old_value != new_value
            }
            None => self.modified.contains_key(field), // CREATE - field exists
        }
    }
    
    /// Check if field was changed by API request (vs observer)
    pub fn field_changed_by_api(&self, field: &str) -> bool {
        self.metadata.api_changes.contains(&field.to_string())
    }
    
    /// Check if field was changed by observer
    pub fn field_changed_by_observer(&self, field: &str) -> Option<&str> {
        self.metadata.observer_changes.get(field).map(|s| s.as_str())
    }
    
    /// Add field validation result
    pub fn add_field_validation(&mut self, field: &str, validator: &str, result: ValidationResult) {
        self.metadata.field_validations.insert(
            field.to_string(),
            FieldValidationResult {
                field: field.to_string(),
                validator: validator.to_string(),
                result,
                message: None,
            }
        );
    }
    
    /// Add security check result
    pub fn add_security_check(&mut self, check_type: &str, passed: bool, reason: Option<String>) {
        self.metadata.security_checks.push(SecurityCheck {
            check_type: check_type.to_string(),
            passed,
            reason,
        });
    }
    
    /// Generate SQL for database operations based on changes
    pub fn to_sql_operation(&self, table_name: &str) -> Result<SqlOperation, RecordError> {
        match self.operation {
            RecordOperation::Create => {
                let changes = self.calculate_changes();
                if changes.added.is_empty() {
                    return Err(RecordError::NoChanges("No fields to insert".to_string()));
                }
                
                let fields: Vec<String> = changes.added.keys().cloned().collect();
                let values: Vec<Value> = fields.iter()
                    .map(|field| self.modified[field].clone())
                    .collect();
                
                Ok(SqlOperation::Insert {
                    table: table_name.to_string(),
                    fields,
                    values,
                })
            }
            
            RecordOperation::Update => {
                let changes = self.calculate_changes();
                if !changes.has_changes {
                    return Ok(SqlOperation::NoOp);
                }
                
                let id = self.id.ok_or_else(|| 
                    RecordError::MissingId("UPDATE operation requires record ID".to_string())
                )?;
                
                // Only update fields that actually changed
                let mut update_fields = HashMap::new();
                
                // Include modified fields
                for (field, change) in changes.modified {
                    if let Some(new_value) = change.new_value {
                        update_fields.insert(field, new_value);
                    }
                }
                
                // Include added fields
                for (field, change) in changes.added {
                    if let Some(new_value) = change.new_value {
                        update_fields.insert(field, new_value);
                    }
                }
                
                Ok(SqlOperation::Update {
                    table: table_name.to_string(),
                    id,
                    fields: update_fields,
                })
            }
            
            RecordOperation::Delete => {
                let id = self.id.ok_or_else(|| 
                    RecordError::MissingId("DELETE operation requires record ID".to_string())
                )?;
                
                Ok(SqlOperation::SoftDelete {
                    table: table_name.to_string(),
                    id,
                })
            }
            
            RecordOperation::Revert => {
                let id = self.id.ok_or_else(|| 
                    RecordError::MissingId("REVERT operation requires record ID".to_string())
                )?;
                
                Ok(SqlOperation::Revert {
                    table: table_name.to_string(),
                    id,
                })
            }
            
            RecordOperation::NoChange => Ok(SqlOperation::NoOp),
        }
    }
}

/// Detailed change information for a record
#[derive(Debug, Clone)]
pub struct RecordChanges {
    pub has_changes: bool,
    pub added: HashMap<String, FieldChange>,
    pub modified: HashMap<String, FieldChange>,
    pub removed: Vec<FieldChange>,
}

/// Information about a specific field change
#[derive(Debug, Clone)]
pub struct FieldChange {
    pub field: String,
    pub old_value: Option<Value>,
    pub new_value: Option<Value>,
    pub change_type: ChangeType,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ChangeType {
    Added,
    Modified,
    Removed,
}

/// SQL operations generated from record changes
#[derive(Debug, Clone)]
pub enum SqlOperation {
    Insert {
        table: String,
        fields: Vec<String>,
        values: Vec<Value>,
    },
    Update {
        table: String,
        id: Uuid,
        fields: HashMap<String, Value>,
    },
    SoftDelete {
        table: String,
        id: Uuid,
    },
    Revert {
        table: String,
        id: Uuid,
    },
    NoOp,
}

#[derive(Debug, thiserror::Error)]
pub enum RecordError {
    #[error("Missing record ID: {0}")]
    MissingId(String),
    
    #[error("No changes detected: {0}")]
    NoChanges(String),
    
    #[error("Invalid operation: {0}")]
    InvalidOperation(String),
}
```

## Updated Observer Context

```rust
// src/observer/context.rs - Updated to use StatefulRecord
pub struct ObserverContext {
    // Core request data
    pub system: SystemContext,
    pub operation: Operation,
    pub schema_name: String,
    pub schema: Schema,
    
    // Stateful records instead of raw JSON
    pub records: Vec<StatefulRecord>,
    
    // Global filter for SELECT operations
    pub filter: Option<FilterData>,
    
    // Type-safe metadata storage (unchanged)
    metadata: HashMap<TypeId, Box<dyn Any + Send>>,
    
    // Performance tracking
    pub start_time: std::time::Instant,
    pub current_ring: Option<ObserverRing>,
    
    // Global error accumulation (in addition to per-record validation)
    pub errors: Vec<ObserverError>,
    pub warnings: Vec<ObserverWarning>,
}

impl ObserverContext {
    /// Get records that have specific field changes
    pub fn records_with_field_changes(&self, field: &str) -> Vec<&StatefulRecord> {
        self.records.iter()
            .filter(|record| record.field_changed(field))
            .collect()
    }
    
    /// Get records changed by API vs observers
    pub fn records_changed_by_api(&self) -> Vec<&StatefulRecord> {
        self.records.iter()
            .filter(|record| !record.metadata.api_changes.is_empty())
            .collect()
    }
    
    /// Get all field changes across all records
    pub fn all_field_changes(&self) -> HashMap<String, Vec<FieldChange>> {
        let mut all_changes = HashMap::new();
        
        for record in &self.records {
            let changes = record.calculate_changes();
            
            for (field, change) in changes.added {
                all_changes.entry(field).or_insert_with(Vec::new).push(change);
            }
            
            for (field, change) in changes.modified {
                all_changes.entry(field).or_insert_with(Vec::new).push(change);
            }
            
            for change in changes.removed {
                all_changes.entry(change.field.clone()).or_insert_with(Vec::new).push(change);
            }
        }
        
        all_changes
    }
}
```

## Enhanced Observer Implementations

### 1. RecordPreloader - Loads Original State

```rust
// src/observer/implementations/record_preloader.rs
#[async_trait]
impl DataPreparationObserver for RecordPreloader {
    async fn execute(&self, ctx: &mut ObserverContext) -> Result<(), ObserverError> {
        // Extract IDs that need original state loading
        let record_ids: Vec<Uuid> = ctx.records.iter()
            .filter_map(|record| record.id)
            .collect();
        
        if record_ids.is_empty() {
            tracing::info!("No record IDs found for preloading");
            return Ok(());
        }

        tracing::info!("Preloading {} existing records", record_ids.len());

        // Single database query to fetch all needed existing records
        let repo = Repository::new(&ctx.schema_name, &ctx.system.db_context);
        let filter_data = FilterData {
            where_clause: Some(json!({ "id": { "$in": record_ids } })),
            ..Default::default()
        };
        
        let existing_records = repo.select_any(filter_data).await?;
        let existing_by_id: HashMap<Uuid, Map<String, Value>> = existing_records
            .into_iter()
            .filter_map(|record| {
                let id_str = record.get("id")?.as_str()?;
                let uuid = Uuid::parse_str(id_str).ok()?;
                Some((uuid, record))
            })
            .collect();
        
        // Update StatefulRecords with their original state
        for record in &mut ctx.records {
            if let Some(id) = record.id {
                if let Some(original) = existing_by_id.get(&id) {
                    record.original = Some(original.clone());
                    tracing::debug!("Loaded original state for record {}", id);
                } else {
                    // Record doesn't exist in database
                    match record.operation {
                        RecordOperation::Update | RecordOperation::Delete => {
                            ctx.errors.push(ObserverError::ValidationError(
                                format!("Record {} not found for {:?} operation", id, record.operation)
                            ));
                        }
                        _ => {} // CREATE operations don't need existing records
                    }
                }
            }
        }
        
        tracing::info!("Successfully loaded original state for {} records", existing_by_id.len());
        Ok(())
    }
}
```

### 2. Enhanced Validators Using Field Changes

```rust
// src/observer/implementations/json_schema_validator.rs
#[async_trait]
impl InputValidationObserver for JsonSchemaValidator {
    async fn execute(&self, ctx: &mut ObserverContext) -> Result<(), ObserverError> {
        for record in &mut ctx.records {
            // For UPDATE operations, only validate changed fields
            let fields_to_validate = match record.operation {
                RecordOperation::Create => {
                    // Validate all fields for CREATE
                    record.modified.keys().cloned().collect()
                }
                RecordOperation::Update => {
                    // Only validate fields that changed
                    let changes = record.calculate_changes();
                    let mut changed_fields = Vec::new();
                    changed_fields.extend(changes.added.keys().cloned());
                    changed_fields.extend(changes.modified.keys().cloned());
                    changed_fields
                }
                _ => continue, // Skip validation for DELETE/REVERT
            };
            
            // Validate only the fields that changed (much more efficient!)
            for field in fields_to_validate {
                if let Some(field_value) = record.get_field(&field) {
                    match ctx.schema.validate_field(&field, field_value) {
                        Ok(_) => {
                            record.add_field_validation(
                                &field,
                                "JsonSchemaValidator", 
                                ValidationResult::Valid
                            );
                        }
                        Err(validation_error) => {
                            record.add_field_validation(
                                &field,
                                "JsonSchemaValidator",
                                ValidationResult::Invalid(validation_error.to_string())
                            );
                            
                            ctx.errors.push(ObserverError::ValidationError(
                                format!("Field '{}' validation failed: {}", field, validation_error)
                            ));
                        }
                    }
                }
            }
        }
        
        Ok(())
    }
}

// src/observer/implementations/soft_delete_protector.rs
#[async_trait]
impl SecurityObserver for SoftDeleteProtector {
    async fn execute(&self, ctx: &mut ObserverContext) -> Result<(), ObserverError> {
        for record in &mut ctx.records {
            // Check if record is soft deleted
            if let Some(original) = &record.original {
                let is_trashed = original.get("trashed_at")
                    .map(|v| !v.is_null())
                    .unwrap_or(false);
                
                if is_trashed {
                    record.add_security_check(
                        "soft_delete_protection",
                        false,
                        Some("Record is soft deleted".to_string())
                    );
                    
                    ctx.errors.push(ObserverError::SecurityError(
                        format!("Cannot modify soft deleted record: {}", 
                                record.id.map(|id| id.to_string()).unwrap_or("unknown".to_string()))
                    ));
                } else {
                    record.add_security_check(
                        "soft_delete_protection", 
                        true, 
                        None
                    );
                }
            }
        }
        
        Ok(())
    }
}
```

### 3. Observer That Modifies Records

```rust
// src/observer/implementations/timestamp_enricher.rs
#[derive(Default)]
pub struct TimestampEnricher;

impl Observer for TimestampEnricher {
    fn name(&self) -> &'static str { "TimestampEnricher" }
    fn ring(&self) -> ObserverRing { ObserverRing::Enrichment }
    fn applies_to_operation(&self, op: Operation) -> bool {
        matches!(op, Operation::Create | Operation::Update)
    }
    fn applies_to_schema(&self, _schema: &str) -> bool { true }
}

#[async_trait]
impl EnrichmentObserver for TimestampEnricher {
    async fn execute(&self, ctx: &mut ObserverContext) -> Result<(), ObserverError> {
        let now = Utc::now().to_rfc3339();
        
        for record in &mut ctx.records {
            match record.operation {
                RecordOperation::Create => {
                    // Set both created_at and updated_at for new records
                    if !record.modified.contains_key("created_at") {
                        record.set_field("created_at", Value::String(now.clone()), "TimestampEnricher");
                    }
                    if !record.modified.contains_key("updated_at") {
                        record.set_field("updated_at", Value::String(now.clone()), "TimestampEnricher");
                    }
                }
                
                RecordOperation::Update => {
                    // Only set updated_at for modified records (and only if data actually changed)
                    let changes = record.calculate_changes();
                    if changes.has_changes {
                        record.set_field("updated_at", Value::String(now.clone()), "TimestampEnricher");
                        
                        tracing::debug!("Set updated_at timestamp for record {}", 
                                      record.id.map(|id| id.to_string()).unwrap_or("new".to_string()));
                    }
                }
                
                _ => {} // No timestamp changes for DELETE/REVERT
            }
        }
        
        Ok(())
    }
}
```

### 4. Efficient Database Observer

```rust
// src/observer/implementations/sql_operations.rs
#[async_trait]
impl DatabaseObserver for SqlOperationExecutor {
    async fn execute(&self, ctx: &mut ObserverContext) -> Result<(), ObserverError> {
        let mut results = Vec::new();
        
        for record in &ctx.records {
            // Generate SQL operation based on record changes
            let sql_op = record.to_sql_operation(&ctx.schema_name)?;
            
            let result = match sql_op {
                SqlOperation::Insert { table, fields, values } => {
                    tracing::info!("Inserting record into {}: fields={:?}", table, fields);
                    
                    // Build parameterized INSERT query
                    let placeholders = (1..=fields.len())
                        .map(|i| format!("${}", i))
                        .collect::<Vec<_>>()
                        .join(", ");
                    
                    let field_list = fields.iter()
                        .map(|f| format!("\"{}\"", f))
                        .collect::<Vec<_>>()
                        .join(", ");
                    
                    let query = format!(
                        "INSERT INTO \"{}\" ({}) VALUES ({}) RETURNING *",
                        table, field_list, placeholders
                    );
                    
                    self.execute_query(&ctx.system.db_context, &query, values).await?
                }
                
                SqlOperation::Update { table, id, fields } => {
                    if fields.is_empty() {
                        tracing::debug!("No changes for record {}, skipping update", id);
                        continue;
                    }
                    
                    tracing::info!("Updating record {} in {}: fields={:?}", id, table, fields.keys().collect::<Vec<_>>());
                    
                    // Build SET clause for only changed fields
                    let set_clauses: Vec<String> = fields.keys()
                        .enumerate()
                        .map(|(i, field)| format!("\"{}\" = ${}", field, i + 1))
                        .collect();
                    
                    let values: Vec<Value> = fields.values().cloned().collect();
                    let mut all_values = values;
                    all_values.push(Value::String(id.to_string()));
                    
                    let query = format!(
                        "UPDATE \"{}\" SET {} WHERE id = ${} RETURNING *",
                        table, set_clauses.join(", "), all_values.len()
                    );
                    
                    self.execute_query(&ctx.system.db_context, &query, all_values).await?
                }
                
                SqlOperation::SoftDelete { table, id } => {
                    tracing::info!("Soft deleting record {} from {}", id, table);
                    
                    let query = format!(
                        "UPDATE \"{}\" SET trashed_at = NOW() WHERE id = $1 RETURNING *",
                        table
                    );
                    
                    self.execute_query(
                        &ctx.system.db_context, 
                        &query, 
                        vec![Value::String(id.to_string())]
                    ).await?
                }
                
                SqlOperation::NoOp => {
                    tracing::debug!("No operation needed for record");
                    continue;
                }
                
                _ => continue,
            };
            
            results.push(result);
        }
        
        // Update context with database results
        ctx.result = Some(results);
        
        Ok(())
    }
}
```

## Usage in Route Handlers

```rust
// src/handlers/protected/data/schema_post.rs
pub async fn schema_post(
    Path(schema): Path<String>,
    Extension(ctx): Extension<RequestContext>,
    Json(records_data): Json<Vec<serde_json::Value>>,
) -> Result<ApiResponse<Vec<serde_json::Value>>, ApiError> {
    
    // Convert API data to StatefulRecords
    let stateful_records: Vec<StatefulRecord> = records_data.into_iter()
        .map(|data| {
            if let Value::Object(map) = data {
                StatefulRecord::create(map)
            } else {
                // Handle error case
                StatefulRecord::create(serde_json::Map::new())
            }
        })
        .collect();
    
    // Execute observer pipeline with stateful records
    let pipeline = ObserverPipeline::new();
    let observer_result = pipeline.execute_with_stateful_records(
        ctx.system.clone(),
        Operation::Create,
        schema_name,
        stateful_records,
    ).await?;
    
    if !observer_result.success {
        return Err(ApiError::validation_error(
            format!("Validation failed: {} errors", observer_result.errors.len())
        ));
    }
    
    // Extract results from stateful records
    let created_records: Vec<Value> = observer_result.records.into_iter()
        .map(|record| Value::Object(record.modified))
        .collect();
    
    Ok(ApiResponse::success(created_records))
}

// UPDATE operation example
pub async fn schema_put(
    Path(schema): Path<String>,
    Extension(ctx): Extension<RequestContext>,
    Json(updates): Json<Vec<UpdateRequest>>,
) -> Result<ApiResponse<Vec<serde_json::Value>>, ApiError> {
    
    // Convert API updates to StatefulRecords
    let stateful_records: Vec<StatefulRecord> = updates.into_iter()
        .map(|update| {
            StatefulRecord::existing(
                serde_json::Map::new(), // Will be populated by RecordPreloader
                Some(update.changes),
                RecordOperation::Update
            )
        })
        .collect();
    
    let pipeline = ObserverPipeline::new();
    let observer_result = pipeline.execute_with_stateful_records(
        ctx.system.clone(),
        Operation::Update,
        schema_name,
        stateful_records,
    ).await?;
    
    // Results include only records that actually changed
    let updated_records: Vec<Value> = observer_result.records.into_iter()
        .filter(|record| record.calculate_changes().has_changes)
        .map(|record| Value::Object(record.modified))
        .collect();
    
    Ok(ApiResponse::success(updated_records))
}
```

## Key Benefits of Stateful Record Pattern

### 1. **Precise Change Detection**
```rust
// Know exactly what changed
let changes = record.calculate_changes();
if changes.modified.contains_key("email") {
    // Email was changed - trigger email verification
}
```

### 2. **Efficient Database Operations**
```rust
// Only update fields that actually changed
UPDATE users SET email = $1, updated_at = $2 WHERE id = $3
// Instead of updating all fields
```

### 3. **Granular Validation**
```rust
// Validate only changed fields
for field in record.get_changed_fields() {
    schema.validate_field(&field, record.get_field(&field))?;
}
```

### 4. **Automatic Audit Trail**
```rust
// Track who changed what
record.metadata.observer_changes.get("email") // "TimestampEnricher"
record.metadata.api_changes.contains("email") // true if changed by API
```

### 5. **Smart Observer Logic**
```rust
// Observers can make intelligent decisions
if record.field_changed_by_api("password") {
    // Hash the new password
    record.set_field("password_hash", hash_password(new_password), "PasswordHasher");
    record.remove_field("password", "PasswordHasher");
}
```

This pattern transforms the observer system from a simple data pipeline into an intelligent, change-aware processing system that's both efficient and precisely trackable.
