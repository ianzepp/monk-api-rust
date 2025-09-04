use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Column {
    pub id: Uuid,
    pub access_read: Vec<Uuid>,
    pub access_edit: Vec<Uuid>,
    pub access_full: Vec<Uuid>,
    pub access_deny: Vec<Uuid>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub trashed_at: Option<DateTime<Utc>>,
    pub deleted_at: Option<DateTime<Utc>>,
    pub schema_name: String,
    pub column_name: String,
    pub pg_type: String,
    pub is_required: String,
    pub default_value: Option<String>,
    pub relationship_type: Option<String>,
    pub related_schema: Option<String>,
    pub related_column: Option<String>,
    pub relationship_name: Option<String>,
    pub cascade_delete: Option<bool>,
    pub required_relationship: Option<bool>,
    pub minimum: Option<rust_decimal::Decimal>,
    pub maximum: Option<rust_decimal::Decimal>,
    pub pattern_regex: Option<String>,
    pub enum_values: Option<Vec<String>>,
    pub is_array: Option<bool>,
    pub description: Option<String>,
}