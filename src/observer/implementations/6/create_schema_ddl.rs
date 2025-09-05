// Ring 6: Create Schema DDL Executor - handles CREATE TABLE after schema record insert
use async_trait::async_trait;
use serde_json::{Value, Map};
use sqlx::PgPool;
use uuid::Uuid;

use crate::observer::traits::{Observer, Ring6, ObserverRing, Operation};
use crate::observer::context::ObserverContext;
use crate::observer::error::ObserverError;

/// Ring 6: Create Schema DDL Executor - executes CREATE TABLE when schema record is inserted
#[derive(Default)]
pub struct CreateSchemaDdl;

impl Observer for CreateSchemaDdl {
    fn name(&self) -> &'static str { 
        "CreateSchemaDdl" 
    }
    
    fn ring(&self) -> ObserverRing { 
        ObserverRing::PostDatabase 
    }
    
    fn applies_to_operation(&self, op: Operation) -> bool {
        matches!(op, Operation::Create)
    }
    
    fn applies_to_schema(&self, schema: &str) -> bool {
        schema == "schemas" // Only apply to operations on the schemas table
    }
}

#[async_trait]
impl Ring6 for CreateSchemaDdl {
    async fn execute(&self, context: &mut ObserverContext) -> Result<(), ObserverError> {
        // Get the newly inserted schema record from context
        let records = &context.records
            .ok_or_else(|| ObserverError::ValidationError("No records in context".to_string()))?;

        for record in records {
            // Extract schema information from the inserted record
            let schema_name = record.get("name")
                .and_then(|v| v.as_str())
                .ok_or_else(|| ObserverError::ValidationError("Schema name missing from record".to_string()))?;
                
            let table_name = record.get("table_name")
                .and_then(|v| v.as_str())
                .unwrap_or(schema_name);
                
            let definition = record.get("definition")
                .ok_or_else(|| ObserverError::ValidationError("Schema definition missing from record".to_string()))?;

            // Generate CREATE TABLE DDL from schema definition
            let ddl = self.generate_create_table_ddl(table_name, definition)?;
            
            // Execute DDL
            let pool = context.get_pool()
                .ok_or_else(|| ObserverError::ValidationError("Database pool not available".to_string()))?;
                
            sqlx::query(&ddl)
                .execute(pool)
                .await
                .map_err(|e| ObserverError::DatabaseError(format!("Failed to create table {}: {}", table_name, e)))?;
                
            tracing::info!("Created table '{}' for schema '{}'", table_name, schema_name);
        }

        Ok(())
    }
}

impl CreateSchemaDdl {
    fn generate_create_table_ddl(&self, table_name: &str, definition: &Value) -> Result<String, ObserverError> {
        // Parse the JSON Schema definition
        let schema_def = definition.as_object()
            .ok_or_else(|| ObserverError::ValidationError("Invalid schema definition format".to_string()))?;
            
        let properties = schema_def.get("properties")
            .and_then(|p| p.as_object())
            .ok_or_else(|| ObserverError::ValidationError("Schema properties missing".to_string()))?;
            
        let required = schema_def.get("required")
            .and_then(|r| r.as_array())
            .map(|arr| arr.iter().filter_map(|v| v.as_str()).collect::<Vec<_>>())
            .unwrap_or_default();

        let mut ddl = format!("CREATE TABLE \"{}\" (\n", table_name);

        // Standard system fields
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
        for (field_name, property) in properties {
            // Skip system fields
            if ["id", "access_read", "access_edit", "access_full", "access_deny", 
                "created_at", "updated_at", "trashed_at", "deleted_at"].contains(&field_name.as_str()) {
                continue;
            }

            let pg_type = self.json_schema_type_to_postgres(property)?;
            let is_required = required.contains(&field_name.as_str());
            let nullable = if is_required { " NOT NULL" } else { "" };

            let default_value = if let Some(default) = property.get("default") {
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
    
    fn json_schema_type_to_postgres(&self, property: &Value) -> Result<&str, ObserverError> {
        let property_type = property.get("type")
            .and_then(|t| t.as_str())
            .ok_or_else(|| ObserverError::ValidationError("Property type missing".to_string()))?;
            
        let pg_type = match property_type {
            "string" => {
                if let Some(format) = property.get("format").and_then(|f| f.as_str()) {
                    match format {
                        "uuid" => "UUID",
                        "date-time" => "TIMESTAMP",
                        _ => "TEXT"
                    }
                } else if property.get("enum").is_some() {
                    "TEXT"
                } else if let Some(max_len) = property.get("maxLength").and_then(|m| m.as_i64()) {
                    if max_len <= 255 {
                        "VARCHAR(255)" // Simplified for now
                    } else {
                        "TEXT"
                    }
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
        };
        
        Ok(pg_type)
    }
}