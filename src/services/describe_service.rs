use sqlx::PgPool;
use serde::{Serialize, Deserialize};
use serde_json::Value;
use uuid::Uuid;
use anyhow::Result;
use std::collections::HashMap;

use crate::database::repository::Repository;
use crate::database::record::Record;
use crate::database::manager::DatabaseError;

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

// System fields that are automatically added to all tables
pub const SYSTEM_FIELDS: &[&str] = &[
    "id",
    "access_read",
    "access_edit", 
    "access_full",
    "access_deny",
    "created_at",
    "updated_at",
    "trashed_at",
    "deleted_at",
];

pub struct DescribeService {
    pool: PgPool,
}

impl DescribeService {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Create new schema from JSON content
    pub async fn create_one(&self, schema_name: &str, json_content: Value) -> Result<Record, DescribeError> {
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
        
        // Start transaction for atomic operation
        let mut tx = self.pool.begin().await.map_err(DatabaseError::Sqlx)?;
        
        // Generate and execute DDL
        let ddl = self.generate_create_table_ddl(table_name, &json_schema)?;
        sqlx::query(&ddl).execute(&mut *tx).await.map_err(DatabaseError::Sqlx)?;
        
        // Create schema record
        let json_checksum = self.generate_json_checksum(&json_content.to_string());
        let mut schema_record = Record::new();
        schema_record
            .set("name", schema_name)
            .set("table_name", table_name)
            .set("status", "active")
            .set("definition", serde_json::to_value(&json_schema)?)
            .set("field_count", json_schema.properties.len().to_string())
            .set("json_checksum", json_checksum);
        
        // Insert schema using Repository (we'll need to handle transaction separately for now)
        // For now, let's commit transaction and use Repository
        tx.commit().await.map_err(DatabaseError::Sqlx)?;
        
        let created_schema = schemas_repo.create_one(schema_record).await?;
        
        // Insert column records
        let columns_repo = Repository::new("columns", self.pool.clone());
        self.create_column_records(&columns_repo, schema_name, &json_schema).await?;
        
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
        self.select_one(schema_name).await?
            .ok_or_else(|| DescribeError::NotFound(schema_name.to_string()))
    }

    /// Update existing schema from JSON content
    pub async fn update_404(&self, schema_name: &str, json_content: Value) -> Result<Record, DescribeError> {
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
        let existing_schema = results.into_iter().next()
            .ok_or_else(|| DescribeError::NotFound(schema_name.to_string()))?;
            
        // Update using the schema ID
        let schema_id = existing_schema.id().ok_or_else(|| 
            DescribeError::InvalidFormat("Schema missing ID".to_string()))?;
        
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
        let change = Record::new()
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

    // Private helper methods
    
    async fn schema_exists(&self, pool: &PgPool, schema_name: &str) -> Result<bool, DescribeError> {
        let result = sqlx::query("SELECT COUNT(*) as count FROM schemas WHERE name = $1 AND deleted_at IS NULL")
            .bind(schema_name)
            .fetch_one(pool)
            .await?;
        
        let count: i64 = result.get("count");
        Ok(count > 0)
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
            return Err(DescribeError::InvalidFormat("Schema must have title and properties".to_string()));
        }
        
        Ok(schema)
    }
    
    fn generate_json_checksum(&self, json_content: &str) -> String {
        use sha2::{Sha256, Digest};
        let mut hasher = Sha256::new();
        hasher.update(json_content.as_bytes());
        format!("{:x}", hasher.finalize())
    }
    
    fn generate_create_table_ddl(&self, table_name: &str, json_schema: &JsonSchema) -> Result<String, DescribeError> {
        let empty_vec = vec![];
        let required = json_schema.required.as_ref().unwrap_or(&empty_vec);
        
        let mut ddl = format!("CREATE TABLE \"{}\" (\n", table_name);
        
        // Standard PaaS fields
        ddl += "    \"id\" UUID PRIMARY KEY DEFAULT gen_random_uuid(),\n";
        ddl += "    \"access_read\" UUID[] DEFAULT '{}',\n";
        ddl += "    \"access_edit\" UUID[] DEFAULT '{}',\n";
        ddl += "    \"access_full\" UUID[] DEFAULT '{}',\n";
        ddl += "    \"access_deny\" UUID[] DEFAULT '{}',\n";
        ddl += "    \"created_at\" TIMESTAMP DEFAULT now() NOT NULL,\n";
        ddl += "    \"updated_at\" TIMESTAMP DEFAULT now() NOT NULL,\n";
        ddl += "    \"trashed_at\" TIMESTAMP,\n";
        ddl += "    \"deleted_at\" TIMESTAMP";
        
        // Schema-specific fields
        for (field_name, property) in &json_schema.properties {
            // Skip system fields
            if SYSTEM_FIELDS.contains(&field_name.as_str()) {
                continue;
            }
            
            let pg_type = self.json_schema_type_to_postgres(property);
            let is_required = required.contains(field_name);
            let nullable = if is_required { " NOT NULL" } else { "" };
            
            let default_value = if let Some(default) = &property.default {
                match default {
                    Value::String(s) => format!(" DEFAULT '{}'", s.replace('\'', "''")),
                    Value::Number(n) => format!(" DEFAULT {}", n),
                    Value::Bool(b) => format!(" DEFAULT {}", b),
                    _ => String::new(),
                }
            } else {
                String::new()
            };
            
            ddl += &format!(",\n    \"{}\" {}{}{}", field_name, pg_type, nullable, default_value);
        }
        
        ddl += "\n);";
        Ok(ddl)
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
    
    async fn insert_schema_record(
        &self,
        tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
        schema_name: &str,
        table_name: &str,
        json_schema: &JsonSchema,
        json_checksum: &str,
    ) -> Result<SchemaInfo, DescribeError> {
        let field_count = json_schema.properties.len();
        
        let row = sqlx::query!(
            r#"
            INSERT INTO schemas
            (name, table_name, status, definition, field_count, json_checksum)
            VALUES ($1, $2, 'active', $3, $4, $5)
            RETURNING id, name, table_name, status, definition, field_count, json_checksum,
                      access_read, access_edit, access_full, access_deny,
                      created_at, updated_at, trashed_at, deleted_at
            "#,
            schema_name,
            table_name,
            serde_json::to_value(json_schema)?,
            field_count.to_string(),
            json_checksum
        )
        .fetch_one(&mut **tx)
        .await?;
        
        Ok(SchemaInfo {
            id: row.id,
            name: row.name,
            table_name: row.table_name,
            status: row.status,
            definition: row.definition,
            field_count: row.field_count,
            json_checksum: row.json_checksum,
            access_read: row.access_read,
            access_edit: row.access_edit,
            access_full: row.access_full,
            access_deny: row.access_deny,
            created_at: row.created_at,
            updated_at: row.updated_at,
            trashed_at: row.trashed_at,
            deleted_at: row.deleted_at,
        })
    }
    
    async fn insert_column_records(
        &self,
        tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
        schema_name: &str,
        json_schema: &JsonSchema,
    ) -> Result<(), DescribeError> {
        let empty_vec = vec![];
        let required_fields = json_schema.required.as_ref().unwrap_or(&empty_vec);
        
        for (column_name, column_definition) in &json_schema.properties {
            let pg_type = self.json_schema_type_to_postgres(column_definition);
            let is_required = if required_fields.contains(column_name) { "true" } else { "false" };
            let default_value = column_definition.default.as_ref().map(|v| v.to_string());
            
            // Extract relationship data
            let (relationship_type, related_schema, related_column, relationship_name, cascade_delete, required_relationship) = 
                if let Some(rel) = &column_definition.x_monk_relationship {
                    (
                        Some(rel.relationship_type.clone()),
                        Some(rel.schema.clone()),
                        Some(rel.column.as_deref().unwrap_or("id").to_string()),
                        Some(rel.name.clone()),
                        rel.cascade_delete,
                        rel.required,
                    )
                } else {
                    (None, None, None, None, None, None)
                };
            
            sqlx::query!(
                r#"
                INSERT INTO columns
                (schema_name, column_name, pg_type, is_required, default_value, 
                 relationship_type, related_schema, related_column, relationship_name, 
                 cascade_delete, required_relationship, minimum, maximum, pattern_regex, 
                 enum_values, is_array, description)
                VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16, $17)
                "#,
                schema_name,
                column_name,
                pg_type,
                is_required,
                default_value,
                relationship_type,
                related_schema,
                related_column,
                relationship_name,
                cascade_delete,
                required_relationship,
                column_definition.minimum.map(|v| BigDecimal::from(v as i64)),
                column_definition.maximum.map(|v| BigDecimal::from(v as i64)),
                column_definition.pattern,
                column_definition.enum_values.as_ref().map(|v| v.as_slice()),
                Some(column_definition.property_type == "array"),
                column_definition.description
            )
            .execute(&mut **tx)
            .await?;
        }
        
        Ok(())
    }
}