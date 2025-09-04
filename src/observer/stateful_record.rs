use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use std::collections::{HashMap, HashSet};
use uuid::Uuid;

/// Operation type for individual records
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RecordOperation {
    Create,
    Update,
    Delete,
    Revert,
    /// For SELECT results and NoOp updates
    NoChange,
}

/// Information about a specific field change
#[derive(Debug, Clone, PartialEq)]
pub struct FieldChange {
    pub field: String,
    pub old_value: Option<Value>,
    pub new_value: Option<Value>,
    pub change_type: ChangeType,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChangeType {
    Added,
    Modified,
    Removed,
}

/// Detailed change information for a record
#[derive(Debug, Clone, Default, PartialEq)]
pub struct RecordChanges {
    pub has_changes: bool,
    pub added: HashMap<String, FieldChange>,
    pub modified: HashMap<String, FieldChange>,
    pub removed: Vec<FieldChange>,
}

/// Validation result metadata
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ValidationResult {
    Valid,
    Invalid(String),
    Warning(String),
}

#[derive(Debug, Clone)]
pub struct FieldValidationResult {
    pub field: String,
    pub validator: String,
    pub result: ValidationResult,
    pub message: Option<String>,
}

#[derive(Debug, Clone)]
pub struct SecurityCheck {
    pub check_type: String,
    pub passed: bool,
    pub reason: Option<String>,
}

/// Processing metadata and validation traces
#[derive(Debug, Clone)]
pub struct RecordMetadata {
    /// Fields modified directly by the API request
    pub api_changes: HashSet<String>,
    /// Fields modified by observers (field -> observer name)
    pub observer_changes: HashMap<String, String>,
    /// Validation results per field
    pub field_validations: HashMap<String, FieldValidationResult>,
    /// Security checks applied to the record
    pub security_checks: Vec<SecurityCheck>,
    /// Timestamp when record entered the pipeline
    pub pipeline_start: DateTime<Utc>,
}

/// Per-record wire-response metadata (serialized as `meta` in API)
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct RecordResponseMetadata {
    pub system: SystemMetadata,
    pub computed: HashMap<String, Value>,
    pub permissions: PermissionMetadata,
    pub relationships: RelationshipMetadata,
    pub processing: ProcessingMetadata,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SystemMetadata {
    pub created_at: Option<DateTime<Utc>>,
    pub updated_at: Option<DateTime<Utc>>,
    pub trashed_at: Option<DateTime<Utc>>,
    pub deleted_at: Option<DateTime<Utc>>,
    pub access_read: Vec<Uuid>,
    pub access_edit: Vec<Uuid>,
    pub access_full: Vec<Uuid>,
    pub access_deny: Vec<Uuid>,
    pub version: Option<i64>,
    pub tenant_id: Option<Uuid>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AccessLevel {
    None,
    Read,
    Edit,
    Full,
    Root,
}

impl Default for AccessLevel {
    fn default() -> Self { Self::None }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PermissionMetadata {
    pub can_read: bool,
    pub can_edit: bool,
    pub can_delete: bool,
    pub can_share: bool,
    pub effective_access_level: AccessLevel,
    pub permission_source: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct RelationshipMetadata {
    pub related_counts: HashMap<String, i64>,
    pub relationships: HashMap<String, Vec<Uuid>>, // schema_name -> ids
    pub parent_schema: Option<String>,
    pub child_schemas: Vec<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ProcessingMetadata {
    pub enriched_by: Vec<String>,
    pub processing_time_ms: Option<u64>,
    pub cache_hit: bool,
    pub query_stats: Option<RecordQueryStats>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecordQueryStats {
    pub execution_time_ms: u64,
    pub rows_examined: u64,
    pub index_used: Option<String>,
}

/// SQL operations generated from record changes
#[derive(Debug, Clone, PartialEq)]
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

/// MVP StatefulRecord implementation
#[derive(Debug, Clone)]
pub struct StatefulRecord {
    /// Unique identifier for this record
    pub id: Option<Uuid>,
    /// Original state from database (None for CREATE operations)
    pub original: Option<Map<String, Value>>,
    /// Current modified state (user fields only)
    pub modified: Map<String, Value>,
    /// Operation type for this record
    pub operation: RecordOperation,
    /// Change tracking metadata
    pub metadata: RecordMetadata,
    /// Structured metadata for API responses (serialized as `meta`)
    pub response_metadata: RecordResponseMetadata,
}

impl Default for RecordMetadata {
    fn default() -> Self {
        Self {
            api_changes: HashSet::new(),
            observer_changes: HashMap::new(),
            field_validations: HashMap::new(),
            security_checks: Vec::new(),
            pipeline_start: Utc::now(),
        }
    }
}

impl StatefulRecord {
    /// Create new record for CREATE operation (API-supplied data)
    pub fn create(mut data: Map<String, Value>) -> Self {
        // Ensure system metadata is extracted out of user attributes if present
        let mut record = Self {
            id: None,
            original: None,
            modified: Map::new(),
            operation: RecordOperation::Create,
            metadata: RecordMetadata::default(),
            response_metadata: RecordResponseMetadata::default(),
        };
        // Treat all incoming fields as API changes
for (k, v) in data.into_iter() {
            record.set_field_api(k, v);
        }
        record
    }

    /// Create existing record for UPDATE/DELETE/REVERT operation
    pub fn existing(
        original: Map<String, Value>,
        changes: Option<Map<String, Value>>,
        operation: RecordOperation,
    ) -> Self {
        let id = original
            .get("id")
            .and_then(|v| v.as_str())
            .and_then(|s| Uuid::parse_str(s).ok());

        let mut record = Self {
            id,
            original: Some(original.clone()),
            modified: original,
            operation,
            metadata: RecordMetadata::default(),
            response_metadata: RecordResponseMetadata::default(),
        };

        if let Some(mut changes_map) = changes {
for (k, v) in changes_map.into_iter() {
                record.set_field_api(k, v);
            }
        }

        record
    }

    /// Get the current value of a field
    pub fn get_field(&self, field: &str) -> Option<&Value> {
        self.modified.get(field)
    }

    /// Set field value from API input
    pub fn set_field_api(&mut self, field: String, value: Value) {
        self.metadata.api_changes.insert(field.clone());
        self.modified.insert(field, value);
    }

    /// Set field value from an observer
    pub fn set_field_observer(&mut self, field: String, value: Value, observer_name: &str) {
        self.modified.insert(field.clone(), value);
        self
            .metadata
            .observer_changes
            .insert(field, observer_name.to_string());
    }

    /// Remove a field entirely (omit from UPDATE); use set_field_* with Value::Null for setting NULL
    pub fn remove_field(&mut self, field: &str, observer_name: &str) {
        self.modified.remove(field);
        self
            .metadata
            .observer_changes
            .insert(field.to_string(), format!("{} (removed)", observer_name));
    }

    /// Extract system metadata fields from `modified` into `response_metadata.system`
    pub fn extract_system_metadata(&mut self) {
        const SYSTEM_FIELDS: &[&str] = &[
            "created_at",
            "updated_at",
            "trashed_at",
            "deleted_at",
            "access_read",
            "access_edit",
            "access_full",
            "access_deny",
            "version",
            "tenant_id",
        ];

        for field in SYSTEM_FIELDS {
            if let Some(value) = self.modified.remove(*field) {
                match *field {
                    "created_at" => self.response_metadata.system.created_at = parse_dt_opt(&value),
                    "updated_at" => self.response_metadata.system.updated_at = parse_dt_opt(&value),
                    "trashed_at" => self.response_metadata.system.trashed_at = parse_dt_opt(&value),
                    "deleted_at" => self.response_metadata.system.deleted_at = parse_dt_opt(&value),
                    "access_read" => self.response_metadata.system.access_read = parse_uuid_array(&value),
                    "access_edit" => self.response_metadata.system.access_edit = parse_uuid_array(&value),
                    "access_full" => self.response_metadata.system.access_full = parse_uuid_array(&value),
                    "access_deny" => self.response_metadata.system.access_deny = parse_uuid_array(&value),
                    "version" => self.response_metadata.system.version = value.as_i64(),
                    "tenant_id" => self.response_metadata.system.tenant_id = value.as_str().and_then(|s| Uuid::parse_str(s).ok()),
                    _ => {}
                }
            }
        }
    }

    /// Calculate diff between original and modified state (top-level keys only)
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
                                },
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
                                },
                            );
                        }
                        _ => {}
                    }
                }

                // Find removed fields
                for key in original.keys() {
                    if !self.modified.contains_key(key) {
                        removed.push(FieldChange {
                            field: key.clone(),
                            old_value: original.get(key).cloned(),
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
                // CREATE operation - all fields are added
                let added = self
                    .modified
                    .iter()
                    .map(|(key, value)| {
                        (
                            key.clone(),
                            FieldChange {
                                field: key.clone(),
                                old_value: None,
                                new_value: Some(value.clone()),
                                change_type: ChangeType::Added,
                            },
                        )
                    })
                    .collect();

                RecordChanges {
                    has_changes: !self.modified.is_empty(),
                    added,
                    modified: HashMap::new(),
                    removed: Vec::new(),
                }
            }
        }
    }

    /// Check if a specific field changed compared to original
    pub fn field_changed(&self, field: &str) -> bool {
        match &self.original {
            Some(original) => original.get(field) != self.modified.get(field),
            None => self.modified.contains_key(field),
        }
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
            },
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
    /// Notes:
    /// - Filters out system fields from INSERT/UPDATE
    /// - Deterministic ordering of SET/VALUES via key sorting
    pub fn to_sql_operation(&self, table_name: &str) -> Result<SqlOperation, RecordError> {
        const SYSTEM_DENYLIST: &[&str] = &[
            "id",
            "created_at",
            "updated_at",
            "deleted_at",
            "trashed_at",
            "access_read",
            "access_edit",
            "access_full",
            "access_deny",
            "version",
        ];

        match self.operation {
            RecordOperation::Create => {
                let changes = self.calculate_changes();
                if changes.added.is_empty() {
                    return Err(RecordError::NoChanges("No fields to insert".to_string()));
                }

                // Filter and sort fields
                let mut fields: Vec<String> = changes
                    .added
                    .keys()
                    .filter(|k| !SYSTEM_DENYLIST.contains(&k.as_str()))
                    .cloned()
                    .collect();
                fields.sort();

                if fields.is_empty() {
                    return Err(RecordError::NoChanges("Only system fields present; nothing to insert".to_string()));
                }

                let values: Vec<Value> = fields
                    .iter()
                    .map(|f| self.modified.get(f).cloned().unwrap_or(Value::Null))
                    .collect();

                Ok(SqlOperation::Insert {
                    table: table_name.to_string(),
                    fields,
                    values,
                })
            }
            RecordOperation::Update => {
                let changes = self.calculate_changes();
                let id = self
                    .id
                    .ok_or_else(|| RecordError::MissingId("UPDATE operation requires record ID".to_string()))?;

                // Build map of only changed fields (added + modified)
                let mut update_fields: HashMap<String, Value> = HashMap::new();
                for (field, change) in changes.modified {
                    if !SYSTEM_DENYLIST.contains(&field.as_str()) {
                        if let Some(v) = change.new_value {
                            update_fields.insert(field, v);
                        }
                    }
                }
                for (field, change) in changes.added {
                    if !SYSTEM_DENYLIST.contains(&field.as_str()) {
                        if let Some(v) = change.new_value {
                            update_fields.insert(field, v);
                        }
                    }
                }

                if update_fields.is_empty() {
                    return Ok(SqlOperation::NoOp);
                }

                Ok(SqlOperation::Update {
                    table: table_name.to_string(),
                    id,
                    fields: update_fields,
                })
            }
            RecordOperation::Delete => {
                let id = self
                    .id
                    .ok_or_else(|| RecordError::MissingId("DELETE operation requires record ID".to_string()))?;
                Ok(SqlOperation::SoftDelete {
                    table: table_name.to_string(),
                    id,
                })
            }
            RecordOperation::Revert => {
                let id = self
                    .id
                    .ok_or_else(|| RecordError::MissingId("REVERT operation requires record ID".to_string()))?;
                Ok(SqlOperation::Revert {
                    table: table_name.to_string(),
                    id,
                })
            }
            RecordOperation::NoChange => Ok(SqlOperation::NoOp),
        }
    }
}

fn parse_dt_opt(value: &Value) -> Option<DateTime<Utc>> {
    value
        .as_str()
        .and_then(|s| chrono::DateTime::parse_from_rfc3339(s).ok())
        .map(|dt| dt.with_timezone(&Utc))
}

fn parse_uuid_array(value: &Value) -> Vec<Uuid> {
    value
        .as_array()
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str())
                .filter_map(|s| Uuid::parse_str(s).ok())
                .collect::<Vec<_>>()
        })
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn map(pairs: Vec<(&str, Value)>) -> Map<String, Value> {
        let mut m = Map::new();
        for (k, v) in pairs { m.insert(k.to_string(), v); }
        m
    }

    #[test]
    fn create_changes_all_added() {
        let m = map(vec![("name", Value::String("Alice".into())), ("age", Value::from(30))]);
        let rec = StatefulRecord::create(m);
        let ch = rec.calculate_changes();
        assert!(ch.has_changes);
        assert_eq!(ch.added.len(), 2);
        assert!(ch.modified.is_empty());
        assert!(ch.removed.is_empty());
        let sql = rec.to_sql_operation("users").unwrap();
        match sql { SqlOperation::Insert{ fields, values, .. } => {
            // Deterministic, sorted keys
            assert_eq!(fields, vec!["age".to_string(), "name".to_string()]);
            assert_eq!(values.len(), 2);
        }, _ => panic!("expected insert") }
    }

    #[test]
    fn update_changes_detected_and_filtered() {
        let original = map(vec![
            ("id", Value::String("11111111-1111-1111-1111-111111111111".into())),
            ("name", Value::String("Alice".into())),
            ("created_at", Value::String("2024-01-01T00:00:00Z".into())),
        ]);
        let changes = map(vec![
            ("name", Value::String("Alice B".into())),
            ("created_at", Value::String("2025-01-01T00:00:00Z".into())), // system field should be dropped
        ]);
        let rec = StatefulRecord::existing(original, Some(changes), RecordOperation::Update);
        let sql = rec.to_sql_operation("users").unwrap();
        match sql { SqlOperation::Update{ fields, .. } => {
            assert!(fields.contains_key("name"));
            assert!(!fields.contains_key("created_at"));
        }, _ => panic!("expected update") }
    }

    #[test]
    fn null_vs_remove_semantics() {
        let original = map(vec![
            ("id", Value::String("22222222-2222-2222-2222-222222222222".into())),
            ("nickname", Value::String("Al".into())),
        ]);
        let mut rec = StatefulRecord::existing(original, None, RecordOperation::Update);
        // Set to null -> should produce update setting NULL
        rec.set_field_api("nickname".to_string(), Value::Null);
        let sql = rec.to_sql_operation("users").unwrap();
        match sql { SqlOperation::Update{ fields, .. } => {
            assert!(fields.contains_key("nickname"));
            assert!(fields.get("nickname").unwrap().is_null());
        }, _ => panic!("expected update") }

        // Remove field -> omit from update
        let mut rec2 = rec.clone();
        rec2.remove_field("nickname", "tester");
        let sql2 = rec2.to_sql_operation("users").unwrap();
        match sql2 { SqlOperation::Update{ fields, .. } => {
            assert!(!fields.contains_key("nickname"));
        }, _ => panic!("expected update") }
    }
}
