use chrono::{DateTime, Utc};
use serde_json::{Map, Value};
use std::collections::{HashMap, HashSet};
use uuid::Uuid;

/// System fields that can only be set by observers, not by API input
const SYSTEM_FIELDS: &[&str] = &[
    "id",
    "created_at",
    "updated_at",
    "trashed_at",
    "deleted_at",
    "access_read",
    "access_write",
    "access_delete",
];

/// Operation type for record processing
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Operation {
    Create,
    Update,
    Delete,
    Select,
}

/// Field change information for diff tracking
#[derive(Debug, Clone, PartialEq)]
pub struct FieldChange {
    pub field: String,
    pub old_value: Option<Value>,
    pub new_value: Option<Value>,
    pub change_type: ChangeType,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChangeType {
    Added,    // Field didn't exist in original
    Modified, // Field existed but value changed
    Removed,  // Field was explicitly removed
}

/// Complete diff information for a record
#[derive(Debug, Clone)]
pub struct RecordDiff {
    pub added: HashMap<String, Value>,
    pub modified: HashMap<String, FieldChange>,
    pub removed: HashSet<String>,
    pub unchanged: HashSet<String>,
}

/// Errors that can occur during Record operations
#[derive(Debug, thiserror::Error)]
pub enum RecordError {
    #[error("System field '{0}' cannot be set via API input")]
    SystemFieldNotAllowed(&'static str),
    #[error("Invalid JSON format: {0}")]
    InvalidJson(String),
    #[error("Missing required field: {0}")]
    MissingRequiredField(String),
    #[error("Invalid UUID format for field '{field}': {value}")]
    InvalidUuid { field: String, value: String },
    #[error("Invalid timestamp format for field '{field}': {value}")]
    InvalidTimestamp { field: String, value: String },
}

/// A dynamic record that can represent any database row with change tracking
#[derive(Debug, Clone)]
pub struct Record {
    /// Original state from database (None for CREATE operations)
    original: Option<HashMap<String, Value>>,
    /// Current field values
    fields: HashMap<String, Value>,
    /// Fields that have been modified since original
    modified_fields: HashSet<String>,
    /// Current operation type
    operation: Operation,
}

impl Default for Record {
    fn default() -> Self {
        Self::new()
    }
}

impl Record {
    /// Create a new empty record
    pub fn new() -> Self {
        Self {
            original: None,
            fields: HashMap::new(),
            modified_fields: HashSet::new(),
            operation: Operation::Create,
        }
    }

    /// Create record from API input JSON, filtering out system fields
    pub fn from_json(json: Value) -> Result<Self, RecordError> {
        let mut record = Self::new();

        match json {
            Value::Object(map) => {
                for (key, value) in map {
                    // Reject system fields from API input
                    if SYSTEM_FIELDS.contains(&key.as_str()) {
                        return Err(RecordError::SystemFieldNotAllowed(
                            SYSTEM_FIELDS.iter().find(|&&f| f == key).unwrap(),
                        ));
                    }
                    record.fields.insert(key, value);
                }
                Ok(record)
            }
            _ => Err(RecordError::InvalidJson("Expected JSON object".to_string())),
        }
    }

    /// Create record from API input (alias for from_json)
    pub fn from_api_input(json: Value) -> Result<Self, RecordError> {
        Self::from_json(json)
    }

    /// Create record from SQL row data (allows system fields)
    pub fn from_sql_data(data: HashMap<String, Value>) -> Self {
        Self {
            original: Some(data.clone()),
            fields: data,
            modified_fields: HashSet::new(),
            operation: Operation::Select,
        }
    }

    /// Inject original data from SQL loader (for tracking changes)
    pub fn inject(&mut self, original_data: HashMap<String, Value>) -> &mut Self {
        self.original = Some(original_data);
        self.operation = Operation::Update;
        self
    }

    /// Get field value
    pub fn get(&self, key: &str) -> Option<&Value> {
        self.fields.get(key)
    }

    /// Set field value with automatic change tracking
    pub fn set(&mut self, key: impl Into<String>, value: impl Into<Value>) -> &mut Self {
        let key = key.into();

        // Prevent setting system fields directly (observers can use set_system_field)
        if SYSTEM_FIELDS.contains(&key.as_str()) {
            tracing::warn!("Attempted to set system field '{}' - ignoring", key);
            return self;
        }

        // Track changes if we have original data
        if self.original.is_some() {
            self.modified_fields.insert(key.clone());
        }

        self.fields.insert(key, value.into());
        self
    }

    /// Set system field (for observers only)
    pub fn set_system_field(
        &mut self,
        key: impl Into<String>,
        value: impl Into<Value>,
    ) -> &mut Self {
        let key = key.into();

        // Track changes if we have original data
        if self.original.is_some() {
            self.modified_fields.insert(key.clone());
        }

        self.fields.insert(key, value.into());
        self
    }

    /// Remove field and return its value
    pub fn remove(&mut self, key: &str) -> Option<Value> {
        if self.original.is_some() {
            self.modified_fields.insert(key.to_string());
        }
        self.fields.remove(key)
    }

    /// Remove field (chainable)
    pub fn unset(&mut self, key: &str) -> &mut Self {
        self.remove(key);
        self
    }

    /// Apply multiple changes at once
    pub fn apply_changes(&mut self, changes: HashMap<String, Value>) -> &mut Self {
        for (key, value) in changes {
            self.set(key, value);
        }
        self
    }

    /// Merge another record's fields into this one
    pub fn merge(&mut self, other: HashMap<String, Value>) -> &mut Self {
        self.apply_changes(other)
    }

    // ========================================
    // Standard field accessors
    // ========================================

    /// Get record ID
    pub fn id(&self) -> Option<Uuid> {
        self.get("id").and_then(|v| v.as_str()).and_then(|s| Uuid::parse_str(s).ok())
    }

    /// Set record ID (system field)
    pub fn set_id(&mut self, id: Uuid) -> &mut Self {
        self.set_system_field("id", Value::String(id.to_string()))
    }

    /// Get created_at timestamp
    pub fn created_at(&self) -> Option<DateTime<Utc>> {
        self.get("created_at")
            .and_then(|v| v.as_str())
            .and_then(|s| DateTime::parse_from_rfc3339(s).ok())
            .map(|dt| dt.with_timezone(&Utc))
    }

    /// Get updated_at timestamp
    pub fn updated_at(&self) -> Option<DateTime<Utc>> {
        self.get("updated_at")
            .and_then(|v| v.as_str())
            .and_then(|s| DateTime::parse_from_rfc3339(s).ok())
            .map(|dt| dt.with_timezone(&Utc))
    }

    /// Get trashed_at timestamp
    pub fn trashed_at(&self) -> Option<DateTime<Utc>> {
        self.get("trashed_at")
            .and_then(|v| v.as_str())
            .and_then(|s| DateTime::parse_from_rfc3339(s).ok())
            .map(|dt| dt.with_timezone(&Utc))
    }

    /// Check if record is soft deleted
    pub fn is_trashed(&self) -> bool {
        self.trashed_at().is_some()
    }

    /// Touch updated_at field (for observers)
    pub fn touch_updated_at(&mut self) -> &mut Self {
        self.set_system_field("updated_at", Value::String(Utc::now().to_rfc3339()))
    }

    /// Mark record as deleted (soft delete)
    pub fn mark_deleted(&mut self) -> &mut Self {
        self.set_system_field("trashed_at", Value::String(Utc::now().to_rfc3339()));
        self.operation = Operation::Delete;
        self
    }

    /// Unmark record as deleted (revert soft delete)
    pub fn unmark_deleted(&mut self) -> &mut Self {
        self.set_system_field("trashed_at", Value::Null);
        self.operation = Operation::Update;
        self
    }

    // ========================================
    // Change tracking
    // ========================================

    /// Check if a specific field has been changed
    pub fn changed(&self, key: &str) -> bool {
        match (&self.original, self.fields.get(key)) {
            (Some(original), Some(current)) => original.get(key) != Some(current),
            (Some(original), None) => original.contains_key(key),
            (None, Some(_)) => true, // New field on create
            (None, None) => false,
        }
    }

    /// Check if record has any changes
    pub fn has_changes(&self) -> bool {
        !self.modified_fields.is_empty() || self.original.is_none()
    }

    /// Get original data (before changes)
    pub fn original(&self) -> Option<&HashMap<String, Value>> {
        self.original.as_ref()
    }

    /// Get original value for a specific field
    pub fn get_original(&self, key: &str) -> Option<&Value> {
        self.original.as_ref()?.get(key)
    }

    /// Get detailed changes for each field
    pub fn changes(&self) -> HashMap<String, FieldChange> {
        let mut changes = HashMap::new();

        if let Some(original) = &self.original {
            // Check modified and added fields
            for field in &self.modified_fields {
                let old_value = original.get(field).cloned();
                let new_value = self.fields.get(field).cloned();

                let change_type = match (&old_value, &new_value) {
                    (None, Some(_)) => ChangeType::Added,
                    (Some(_), None) => ChangeType::Removed,
                    (Some(old), Some(new)) if old != new => ChangeType::Modified,
                    _ => continue, // No actual change
                };

                changes.insert(
                    field.clone(),
                    FieldChange { field: field.clone(), old_value, new_value, change_type },
                );
            }
        } else {
            // For CREATE operations, all fields are "added"
            for (field, value) in &self.fields {
                changes.insert(
                    field.clone(),
                    FieldChange {
                        field: field.clone(),
                        old_value: None,
                        new_value: Some(value.clone()),
                        change_type: ChangeType::Added,
                    },
                );
            }
        }

        changes
    }

    /// Get comprehensive diff information
    pub fn diff(&self) -> RecordDiff {
        let mut diff = RecordDiff {
            added: HashMap::new(),
            modified: HashMap::new(),
            removed: HashSet::new(),
            unchanged: HashSet::new(),
        };

        if let Some(original) = &self.original {
            // All current fields
            for (key, value) in &self.fields {
                match original.get(key) {
                    None => {
                        diff.added.insert(key.clone(), value.clone());
                    }
                    Some(old_value) if old_value != value => {
                        diff.modified.insert(
                            key.clone(),
                            FieldChange {
                                field: key.clone(),
                                old_value: Some(old_value.clone()),
                                new_value: Some(value.clone()),
                                change_type: ChangeType::Modified,
                            },
                        );
                    }
                    Some(_) => {
                        diff.unchanged.insert(key.clone());
                    }
                }
            }

            // Check for removed fields
            for key in original.keys() {
                if !self.fields.contains_key(key) {
                    diff.removed.insert(key.clone());
                }
            }
        } else {
            // For CREATE operations, all fields are added
            for (key, value) in &self.fields {
                diff.added.insert(key.clone(), value.clone());
            }
        }

        diff
    }

    // ========================================
    // Serialization
    // ========================================

    /// Convert to JSON Value (all fields)
    pub fn to_json(&self) -> Value {
        Value::Object(self.fields.iter().map(|(k, v)| (k.clone(), v.clone())).collect())
    }

    /// Convert to API output format (includes system fields)
    pub fn to_api_output(&self) -> Value {
        let mut output = self.fields.clone();

        // Ensure system fields are included from original if they exist
        if let Some(original) = &self.original {
            for &field in SYSTEM_FIELDS {
                if let Some(value) = original.get(field) {
                    // Don't override if field was explicitly modified
                    if !output.contains_key(field) {
                        output.insert(field.to_string(), value.clone());
                    }
                }
            }
        }

        Value::Object(output.into_iter().collect())
    }

    /// Convert to HashMap
    pub fn to_hashmap(&self) -> HashMap<String, Value> {
        self.fields.clone()
    }

    /// Convert to serde_json::Map
    pub fn to_map(&self) -> Map<String, Value> {
        self.fields.iter().map(|(k, v)| (k.clone(), v.clone())).collect()
    }

    // ========================================
    // Operation management
    // ========================================

    /// Get current operation type
    pub fn operation(&self) -> Operation {
        self.operation
    }

    /// Set operation type
    pub fn set_operation(&mut self, operation: Operation) -> &mut Self {
        self.operation = operation;
        self
    }

    // ========================================
    // Validation helpers
    // ========================================

    /// Validate that required fields are present and not null
    pub fn validate_required_fields(&self, fields: &[&str]) -> Result<(), RecordError> {
        for &field in fields {
            match self.get(field) {
                None => return Err(RecordError::MissingRequiredField(field.to_string())),
                Some(Value::Null) => {
                    return Err(RecordError::MissingRequiredField(field.to_string()))
                }
                Some(_) => continue,
            }
        }
        Ok(())
    }

    /// Get field value or default
    pub fn get_or_default(&self, key: &str, default: Value) -> &Value {
        self.get(key).unwrap_or(&default)
    }

    /// Set field only if it's currently empty/null
    pub fn set_if_empty(&mut self, key: impl Into<String>, value: Value) -> &mut Self {
        let key = key.into();
        match self.get(&key) {
            None | Some(Value::Null) => {
                self.set(key, value);
            }
            Some(_) => {} // Field already has a value
        }
        self
    }
}

// ========================================
// Conversions
// ========================================

impl From<HashMap<String, Value>> for Record {
    fn from(map: HashMap<String, Value>) -> Self {
        Self::from_sql_data(map)
    }
}

impl From<Map<String, Value>> for Record {
    fn from(map: Map<String, Value>) -> Self {
        Self::from_sql_data(map.into_iter().collect())
    }
}

impl From<Record> for HashMap<String, Value> {
    fn from(record: Record) -> Self {
        record.fields
    }
}

impl From<Record> for Value {
    fn from(record: Record) -> Self {
        record.to_json()
    }
}

// ========================================
// Bulk Conversion Helpers
// ========================================

impl Record {
    /// Convert JSON array to Vec<Record> with proper error handling
    pub fn from_json_array(json: Value) -> Result<Vec<Self>, RecordError> {
        match json {
            Value::Array(array) => {
                let mut records = Vec::with_capacity(array.len());
                for (index, item) in array.into_iter().enumerate() {
                    let record = Self::from_json(item)
                        .map_err(|e| RecordError::InvalidJson(format!("Item {}: {}", index, e)))?;
                    records.push(record);
                }
                Ok(records)
            }
            _ => Err(RecordError::InvalidJson("Expected JSON array".to_string())),
        }
    }

    /// Convert Vec<Record> to JSON array
    pub fn to_json_array(records: Vec<Self>) -> Value {
        Value::Array(records.into_iter().map(|r| r.to_json()).collect())
    }

    /// Convert Vec<Record> to API output JSON array
    pub fn to_api_output_array(records: Vec<Self>) -> Value {
        Value::Array(records.into_iter().map(|r| r.to_api_output()).collect())
    }

    /// Try to convert any JSON value to Vec<Record> (handles both single objects and arrays)
    pub fn from_json_flexible(json: Value) -> Result<Vec<Self>, RecordError> {
        match json {
            Value::Array(_) => Self::from_json_array(json),
            Value::Object(_) => Ok(vec![Self::from_json(json)?]),
            _ => Err(RecordError::InvalidJson("Expected JSON object or array".to_string())),
        }
    }
}

// ========================================
// Extension Traits for Ergonomic JSON Handling
// ========================================

/// Extension trait for Vec<Record> to add convenient JSON conversion methods
pub trait RecordVecExt {
    /// Convert to JSON array value
    fn to_json_array(self) -> Value;

    /// Convert to API output JSON array
    fn to_api_output_array(self) -> Value;

    /// Convert to API output (alias for to_api_output_array)
    fn to_api(self) -> Value;

    /// Convert to JSON string
    fn to_json_string(self) -> Result<String, serde_json::Error>;

    /// Convert to API output JSON string
    fn to_api_output_string(self) -> Result<String, serde_json::Error>;
}

impl RecordVecExt for Vec<Record> {
    fn to_json_array(self) -> Value {
        Record::to_json_array(self)
    }

    fn to_api_output_array(self) -> Value {
        Record::to_api_output_array(self)
    }

    fn to_api(self) -> Value {
        self.to_api_output_array()
    }

    fn to_json_string(self) -> Result<String, serde_json::Error> {
        serde_json::to_string(&self.to_json_array())
    }

    fn to_api_output_string(self) -> Result<String, serde_json::Error> {
        serde_json::to_string(&self.to_api_output_array())
    }
}

/// Extension trait for Results containing Vec<Record>
pub trait RecordResultExt<E> {
    /// Map successful results to JSON array
    fn to_json_array(self) -> Result<Value, E>;

    /// Map successful results to API output JSON array
    fn to_api_output_array(self) -> Result<Value, E>;

    /// Map successful results to JSON string
    fn to_json_string(self) -> Result<String, RecordResultError<E>>;

    /// Map successful results to API output JSON string
    fn to_api_output_string(self) -> Result<String, RecordResultError<E>>;
}

impl<E> RecordResultExt<E> for Result<Vec<Record>, E> {
    fn to_json_array(self) -> Result<Value, E> {
        self.map(|records| records.to_json_array())
    }

    fn to_api_output_array(self) -> Result<Value, E> {
        self.map(|records| records.to_api_output_array())
    }

    fn to_json_string(self) -> Result<String, RecordResultError<E>> {
        match self {
            Ok(records) => records.to_json_string().map_err(RecordResultError::SerializationError),
            Err(e) => Err(RecordResultError::OriginalError(e)),
        }
    }

    fn to_api_output_string(self) -> Result<String, RecordResultError<E>> {
        match self {
            Ok(records) => {
                records.to_api_output_string().map_err(RecordResultError::SerializationError)
            }
            Err(e) => Err(RecordResultError::OriginalError(e)),
        }
    }
}

/// Error type for Record result operations
#[derive(Debug)]
pub enum RecordResultError<E> {
    OriginalError(E),
    SerializationError(serde_json::Error),
}

impl<E: std::fmt::Display> std::fmt::Display for RecordResultError<E> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RecordResultError::OriginalError(e) => write!(f, "Original error: {}", e),
            RecordResultError::SerializationError(e) => write!(f, "Serialization error: {}", e),
        }
    }
}

impl<E: std::error::Error + 'static> std::error::Error for RecordResultError<E> {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            RecordResultError::OriginalError(e) => Some(e),
            RecordResultError::SerializationError(e) => Some(e),
        }
    }
}

// ========================================
// Display
// ========================================

impl std::fmt::Display for Record {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Record(id: {:?}, fields: {}, changed: {})",
            self.id(),
            self.fields.len(),
            self.has_changes()
        )
    }
}
