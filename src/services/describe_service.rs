use anyhow::Result;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sqlx::PgPool;

use crate::database::manager::DatabaseError;
use crate::database::record::Record;
use crate::database::repository::Repository;

// Note: SchemaInfo and ColumnInfo are now replaced by Record type

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonSchemaProperty {
    #[serde(rename = "type")]
    pub property_type: String,
    pub format: Option<String>,
    pub pattern: Option<String>,
    #[serde(rename = "enum")]
    pub enum_values: Option<Vec<String>>,
    #[serde(rename = "minLength")]
    pub min_length: Option<i32>,
    #[serde(rename = "maxLength")]
    pub max_length: Option<i32>,
    pub minimum: Option<f64>,
    pub maximum: Option<f64>,
    pub default: Option<Value>,
    pub description: Option<String>,
    #[serde(rename = "x-monk-relationship")]
    pub x_monk_relationship: Option<XMonkRelationship>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct XMonkRelationship {
    #[serde(rename = "type")]
    pub relationship_type: String, // "owned" | "referenced"
    pub schema: String,
    pub name: String,
    pub column: Option<String>,
    #[serde(rename = "cascadeDelete")]
    pub cascade_delete: Option<bool>,
    pub required: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonSchema {
    pub name: String,
    pub title: String,
    pub table: Option<String>,
    pub description: Option<String>,
    pub properties: std::collections::HashMap<String, JsonSchemaProperty>,
    pub required: Option<Vec<String>>,
}

#[derive(Debug, thiserror::Error)]
pub enum DescribeError {
    #[error("Database error: {0}")]
    Database(#[from] DatabaseError),
    #[error("Schema not found: {0}")]
    NotFound(String),
    #[error("Schema already exists: {0}")]
    AlreadyExists(String),
    #[error("Invalid schema format: {0}")]
    InvalidFormat(String),
    #[error("Schema is protected: {0}")]
    Protected(String),
    #[error("JSON parsing error: {0}")]
    JsonParse(#[from] serde_json::Error),
}

pub struct DescribeService {
    pool: PgPool,
}

impl DescribeService {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Create new schema from JSON content
    pub async fn create_one(
        &self,
        schema_name: &str,
        json_content: Value,
    ) -> Result<Record, DescribeError> {
        // Validate schema protection
        self.validate_schema_protection(schema_name)?;

        // Parse and validate JSON Schema
        let json_schema = self.parse_json_schema(json_content.clone())?;
        let table_name = json_schema.table.as_deref().unwrap_or(schema_name);

        // Check if schema already exists using Repository
        let schemas_repo = Repository::new("schemas", self.pool.clone());
        if self.schema_exists(&schemas_repo, schema_name).await? {
            return Err(DescribeError::AlreadyExists(schema_name.to_string()));
        }

        // Create schema record - Ring 6 observer will handle CREATE TABLE DDL automatically
        let json_checksum = self.generate_json_checksum(&json_content.to_string());
        let mut schema_record = Record::new();
        schema_record
            .set("name", schema_name)
            .set("table_name", table_name)
            .set("status", "active")
            .set("definition", serde_json::to_value(&json_schema)?)
            .set("field_count", json_schema.properties.len().to_string())
            .set("json_checksum", json_checksum);

        // Insert schema - CreateSchemaDdl observer will execute CREATE TABLE
        let created_schema = schemas_repo.create_one(schema_record).await?;

        // Insert column records
        let columns_repo = Repository::new("columns", self.pool.clone());
        self.insert_column_records(&columns_repo, schema_name, &json_schema).await?;

        Ok(created_schema)
    }

    /// Get schema by name
    pub async fn select_one(&self, schema_name: &str) -> Result<Option<Record>, DescribeError> {
        use crate::filter::FilterData;

        let schemas_repo = Repository::new("schemas", self.pool.clone());
        let filter = FilterData {
            where_clause: Some(serde_json::json!({ "name": schema_name })),
            ..Default::default()
        };

        let results = schemas_repo.select_any(filter).await?;
        Ok(results.into_iter().next())
    }

    /// Get schema by name, return 404 error if not found
    pub async fn select_404(&self, schema_name: &str) -> Result<Record, DescribeError> {
        self.select_one(schema_name)
            .await?
            .ok_or_else(|| DescribeError::NotFound(schema_name.to_string()))
    }

    /// Update existing schema from JSON content
    pub async fn update_404(
        &self,
        schema_name: &str,
        json_content: Value,
    ) -> Result<Record, DescribeError> {
        // Validate schema protection
        self.validate_schema_protection(schema_name)?;

        // Parse and validate JSON Schema
        let json_schema = self.parse_json_schema(json_content.clone())?;
        let json_checksum = self.generate_json_checksum(&json_content.to_string());

        // Create updates record
        let mut updates = Record::new();
        updates
            .set("definition", serde_json::to_value(&json_schema)?)
            .set("field_count", json_schema.properties.len().to_string())
            .set("json_checksum", json_checksum);

        // Use Repository to update by name
        let schemas_repo = Repository::new("schemas", self.pool.clone());
        use crate::filter::FilterData;
        let filter = FilterData {
            where_clause: Some(serde_json::json!({ "name": schema_name })),
            ..Default::default()
        };

        // Find existing schema
        let results = schemas_repo.select_any(filter).await?;
        let existing_schema = results
            .into_iter()
            .next()
            .ok_or_else(|| DescribeError::NotFound(schema_name.to_string()))?;

        // Update using the schema ID
        let schema_id = existing_schema
            .id()
            .ok_or_else(|| DescribeError::InvalidFormat("Schema missing ID".to_string()))?;

        let updated_schema = schemas_repo.update_404(schema_id, updates).await?;
        Ok(updated_schema)
    }

    /// Delete schema (soft delete)
    pub async fn delete_one(&self, schema_name: &str) -> Result<bool, DescribeError> {
        // Validate schema protection
        self.validate_schema_protection(schema_name)?;

        // Use Repository to soft delete by setting trashed_at
        let schemas_repo = Repository::new("schemas", self.pool.clone());
        use crate::filter::FilterData;
        let filter = FilterData {
            where_clause: Some(serde_json::json!({
                "name": schema_name,
                "deleted_at": null,
                "trashed_at": null
            })),
            ..Default::default()
        };

        // Create change record with soft delete timestamps
        let mut change = Record::new();
        change
            .set("trashed_at", chrono::Utc::now().to_rfc3339())
            .set("updated_at", chrono::Utc::now().to_rfc3339());

        let updated_records = schemas_repo.update_any(filter, change).await?;
        Ok(!updated_records.is_empty())
    }

    /// Delete schema by name, return 404 error if not found
    pub async fn delete_404(&self, schema_name: &str) -> Result<(), DescribeError> {
        let deleted = self.delete_one(schema_name).await?;
        if deleted {
            Ok(())
        } else {
            Err(DescribeError::NotFound(schema_name.to_string()))
        }
    }

    // ========================================
    // COLUMN OPERATIONS
    // ========================================

    /// Create new column for existing schema from JSON Schema property
    pub async fn create_column(
        &self,
        schema_name: &str,
        column_name: &str,
        json_property: Value,
        is_required: bool,
    ) -> Result<Record, DescribeError> {
        // Validate schema protection
        self.validate_schema_protection(schema_name)?;

        // Verify schema exists
        let schemas_repo = Repository::new("schemas", self.pool.clone());
        if !self.schema_exists(&schemas_repo, schema_name).await? {
            return Err(DescribeError::NotFound(format!("Schema '{}' not found", schema_name)));
        }

        // Parse JSON Schema property into JsonSchemaProperty
        let column_definition: JsonSchemaProperty = serde_json::from_value(json_property)?;

        // Check if column already exists
        let columns_repo = Repository::new("columns", self.pool.clone());
        if self.column_exists(&columns_repo, schema_name, column_name).await? {
            return Err(DescribeError::AlreadyExists(format!(
                "Column '{}' already exists in schema '{}'",
                column_name, schema_name
            )));
        }

        // Parse column definition into Record - CreateColumnDdl observer will handle ALTER TABLE ADD COLUMN
        let column_record = self.parse_column_definition(
            schema_name,
            column_name,
            &column_definition,
            is_required,
        )?;

        let created_column = columns_repo.create_one(column_record).await?;
        Ok(created_column)
    }

    /// Get column by schema and name
    pub async fn select_column(
        &self,
        schema_name: &str,
        column_name: &str,
    ) -> Result<Option<Record>, DescribeError> {
        use crate::filter::FilterData;

        let columns_repo = Repository::new("columns", self.pool.clone());
        let filter = FilterData {
            where_clause: Some(serde_json::json!({
                "schema_name": schema_name,
                "column_name": column_name,
                "deleted_at": null
            })),
            ..Default::default()
        };

        let results = columns_repo.select_any(filter).await?;
        Ok(results.into_iter().next())
    }

    /// Get column by schema and name, return 404 error if not found
    pub async fn select_column_404(
        &self,
        schema_name: &str,
        column_name: &str,
    ) -> Result<Record, DescribeError> {
        self.select_column(schema_name, column_name).await?.ok_or_else(|| {
            DescribeError::NotFound(format!(
                "Column '{}' not found in schema '{}'",
                column_name, schema_name
            ))
        })
    }

    /// Update existing column from JSON Schema property
    pub async fn update_column_404(
        &self,
        schema_name: &str,
        column_name: &str,
        json_property: Value,
        is_required: Option<bool>,
    ) -> Result<Record, DescribeError> {
        // Validate schema protection
        self.validate_schema_protection(schema_name)?;

        // Parse JSON Schema property into JsonSchemaProperty
        let column_definition: JsonSchemaProperty = serde_json::from_value(json_property)?;

        // Find existing column
        let existing_column = self.select_column_404(schema_name, column_name).await?;

        // Determine required status - use provided value or keep existing
        let required = is_required.unwrap_or_else(|| {
            existing_column.get("is_required").and_then(|v| v.as_bool()).unwrap_or(false)
        });

        // Parse updated column definition - UpdateColumnDdl observer will handle safe ALTER COLUMN operations
        let updated_record =
            self.parse_column_definition(schema_name, column_name, &column_definition, required)?;

        // Get column ID for update
        let column_id = existing_column
            .id()
            .ok_or_else(|| DescribeError::InvalidFormat("Column record missing ID".to_string()))?;

        let columns_repo = Repository::new("columns", self.pool.clone());
        let updated_column = columns_repo.update_404(column_id, updated_record).await?;
        Ok(updated_column)
    }

    /// Delete column (soft delete)
    pub async fn delete_column(
        &self,
        schema_name: &str,
        column_name: &str,
    ) -> Result<bool, DescribeError> {
        // Validate schema protection
        self.validate_schema_protection(schema_name)?;

        let columns_repo = Repository::new("columns", self.pool.clone());
        use crate::filter::FilterData;
        let filter = FilterData {
            where_clause: Some(serde_json::json!({
                "schema_name": schema_name,
                "column_name": column_name,
                "deleted_at": null,
                "trashed_at": null
            })),
            ..Default::default()
        };

        // Create change record with soft delete timestamps - DeleteColumnDdl observer will handle ALTER TABLE DROP COLUMN
        let mut change = Record::new();
        change
            .set("trashed_at", chrono::Utc::now().to_rfc3339())
            .set("updated_at", chrono::Utc::now().to_rfc3339());

        let updated_records = columns_repo.update_any(filter, change).await?;
        Ok(!updated_records.is_empty())
    }

    /// Delete column by schema and name, return 404 error if not found
    pub async fn delete_column_404(
        &self,
        schema_name: &str,
        column_name: &str,
    ) -> Result<(), DescribeError> {
        let deleted = self.delete_column(schema_name, column_name).await?;
        if deleted {
            Ok(())
        } else {
            Err(DescribeError::NotFound(format!(
                "Column '{}' not found in schema '{}'",
                column_name, schema_name
            )))
        }
    }

    // Private helper methods

    /// Parse a single JSON Schema property into a column Record
    fn parse_column_definition(
        &self,
        schema_name: &str,
        column_name: &str,
        column_definition: &JsonSchemaProperty,
        is_required: bool,
    ) -> Result<Record, DescribeError> {
        let pg_type = self.json_schema_type_to_postgres(column_definition);

        let mut column_record = Record::new();
        column_record
            .set("schema_name", schema_name)
            .set("column_name", column_name)
            .set("pg_type", pg_type)
            .set("json_type", column_definition.property_type)
            .set("is_required", is_required)
            .set("is_array", column_definition.property_type == "array");

        // Store format if present
        if let Some(format) = &column_definition.format {
            column_record.set("format", format.as_str());
        }

        if let Some(default) = &column_definition.default {
            column_record.set("default_value", default.to_string());
        }

        // Handle numeric constraints (minimum/maximum) or string constraints (minLength/maxLength)
        match column_definition.property_type.as_str() {
            "string" => {
                if let Some(min_len) = column_definition.min_length {
                    column_record.set("minimum", min_len as f64);
                }
                if let Some(max_len) = column_definition.max_length {
                    column_record.set("maximum", max_len as f64);
                }
            }
            "integer" | "number" => {
                if let Some(min) = column_definition.minimum {
                    column_record.set("minimum", min);
                }
                if let Some(max) = column_definition.maximum {
                    column_record.set("maximum", max);
                }
            }
            _ => {
                // For other types, store whatever constraints are present
                if let Some(min) = column_definition.minimum {
                    column_record.set("minimum", min);
                }
                if let Some(max) = column_definition.maximum {
                    column_record.set("maximum", max);
                }
            }
        }

        if let Some(pattern) = &column_definition.pattern {
            column_record.set("pattern_regex", pattern.as_str());
        }
        if let Some(enum_vals) = &column_definition.enum_values {
            column_record.set("enum_values", enum_vals.to_vec());
        }
        if let Some(desc) = &column_definition.description {
            column_record.set("description", desc.as_str());
        }

        // Skip x-monk-relationship for now as requested

        Ok(column_record)
    }

    async fn schema_exists(
        &self,
        schemas_repo: &Repository,
        schema_name: &str,
    ) -> Result<bool, DescribeError> {
        use crate::filter::FilterData;

        let filter = FilterData {
            where_clause: Some(serde_json::json!({
                "name": schema_name,
                "deleted_at": null
            })),
            ..Default::default()
        };

        let results = schemas_repo.select_any(filter).await?;
        Ok(!results.is_empty())
    }

    async fn column_exists(
        &self,
        columns_repo: &Repository,
        schema_name: &str,
        column_name: &str,
    ) -> Result<bool, DescribeError> {
        use crate::filter::FilterData;

        let filter = FilterData {
            where_clause: Some(serde_json::json!({
                "schema_name": schema_name,
                "column_name": column_name,
                "deleted_at": null
            })),
            ..Default::default()
        };

        let results = columns_repo.select_any(filter).await?;
        Ok(!results.is_empty())
    }

    fn validate_schema_protection(&self, schema_name: &str) -> Result<(), DescribeError> {
        let protected_schemas = ["schemas", "users", "columns"];
        if protected_schemas.contains(&schema_name) {
            return Err(DescribeError::Protected(schema_name.to_string()));
        }
        Ok(())
    }

    fn parse_json_schema(&self, json_content: Value) -> Result<JsonSchema, DescribeError> {
        if !json_content.is_object() {
            return Err(DescribeError::InvalidFormat("Schema must be an object".to_string()));
        }

        let schema: JsonSchema = serde_json::from_value(json_content)?;

        if schema.title.is_empty() || schema.properties.is_empty() {
            return Err(DescribeError::InvalidFormat(
                "Schema must have title and properties".to_string(),
            ));
        }

        Ok(schema)
    }

    fn generate_json_checksum(&self, json_content: &str) -> String {
        use sha2::{Digest, Sha256};
        let mut hasher = Sha256::new();
        hasher.update(json_content.as_bytes());
        format!("{:x}", hasher.finalize())
    }

    fn json_schema_type_to_postgres(&self, property: &JsonSchemaProperty) -> &str {
        match property.property_type.as_str() {
            "string" => {
                if property.format.as_deref() == Some("uuid") {
                    "UUID"
                } else if property.format.as_deref() == Some("date-time") {
                    "TIMESTAMP"
                } else if property.enum_values.is_some() {
                    "TEXT"
                } else if let Some(max_len) = property.max_length {
                    if max_len <= 255 {
                        return "VARCHAR(255)"; // Simplified for now
                    }
                    "TEXT"
                } else {
                    "TEXT"
                }
            }
            "integer" => "INTEGER",
            "number" => "DECIMAL",
            "boolean" => "BOOLEAN",
            "array" => "JSONB",
            "object" => "JSONB",
            _ => "TEXT",
        }
    }

    async fn insert_column_records(
        &self,
        columns_repo: &Repository,
        schema_name: &str,
        json_schema: &JsonSchema,
    ) -> Result<(), DescribeError> {
        let empty_vec = vec![];
        let required_fields = json_schema.required.as_ref().unwrap_or(&empty_vec);

        for (column_name, column_definition) in &json_schema.properties {
            let is_required = required_fields.contains(column_name);
            let column_record = self.parse_column_definition(
                schema_name,
                column_name,
                column_definition,
                is_required,
            )?;
            columns_repo.create_one(column_record).await?;
        }

        Ok(())
    }
}
