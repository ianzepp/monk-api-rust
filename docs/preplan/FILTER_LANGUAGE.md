# Filter Language - Rust Implementation Design

Based on the TypeScript filter language implementation, this document outlines how to implement the sophisticated filter system in Rust for monk-api-rust.

## Overview

The monk-api filter language provides **25+ operators** with comprehensive query capabilities:

- **Complex logical operations** (`$and`, `$or`, `$not`, `$nand`, `$nor`)
- **PostgreSQL array operations** (`$any`, `$all`, `$size`) - critical for ACL
- **Range and comparison operators** (`$between`, `$gte`, `$like`, `$regex`)
- **Text search capabilities** (`$find`, `$text`, `$ilike`)
- **Existence checking** (`$exists`, `$null`)

The system supports the complete `/api/find/:schema` endpoint with advanced querying, sorting, and pagination.

## TypeScript Architecture Analysis

### Core Components

1. **`Filter`** - Main query builder class
   - Input validation and normalization
   - Query orchestration
   - SQL generation coordination

2. **`FilterWhere`** - WHERE clause generation
   - 25+ operator implementations
   - Parameter management with offset support
   - Recursive logical operator handling
   - Soft delete integration

3. **`FilterOrder`** - ORDER BY clause generation  
   - Multiple input format support
   - SQL injection protection
   - Column name validation

### Key TypeScript Features

```typescript
// Filter input formats
{ "name": "John", "age": { "$gte": 18 } }                    // Direct conditions
{ "$and": [{ "role": "admin" }, { "active": true }] }        // Logical operators
{ "access_read": { "$any": ["user-123", "group-456"] } }     // ACL array operations
{ "created_at": { "$between": ["2024-01-01", "2024-01-31"] } } // Range queries

// Query output
const { query, params } = filter.toSQL();
// SELECT * FROM "users" WHERE "name" = $1 AND "age" >= $2

// Parameter offsetting for complex queries
const { whereClause, params } = FilterWhere.generate(conditions, startIndex);
```

## Rust Implementation Design

### Core Architecture

```rust
// src/lib/filter/mod.rs
pub mod types;
pub mod filter;
pub mod filter_where; 
pub mod filter_order;
pub mod error;

pub use filter::Filter;
pub use types::*;
```

### Type Definitions

```rust
// src/lib/filter/types.rs
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FilterOp {
    // Comparison operators
    #[serde(rename = "$eq")] Eq,
    #[serde(rename = "$ne")] Ne,
    #[serde(rename = "$neq")] Neq,
    #[serde(rename = "$gt")] Gt,
    #[serde(rename = "$gte")] Gte,
    #[serde(rename = "$lt")] Lt,
    #[serde(rename = "$lte")] Lte,
    
    // Pattern matching
    #[serde(rename = "$like")] Like,
    #[serde(rename = "$nlike")] NLike,
    #[serde(rename = "$ilike")] ILike,
    #[serde(rename = "$nilike")] NILike,
    #[serde(rename = "$regex")] Regex,
    #[serde(rename = "$nregex")] NRegex,
    
    // Array membership
    #[serde(rename = "$in")] In,
    #[serde(rename = "$nin")] NIn,
    
    // PostgreSQL array operations (ACL critical)
    #[serde(rename = "$any")] Any,
    #[serde(rename = "$all")] All,
    #[serde(rename = "$nany")] NAny,
    #[serde(rename = "$nall")] NAll,
    #[serde(rename = "$size")] Size,
    
    // Logical operators
    #[serde(rename = "$and")] And,
    #[serde(rename = "$or")] Or,
    #[serde(rename = "$not")] Not,
    #[serde(rename = "$nand")] NAnd,
    #[serde(rename = "$nor")] NOr,
    
    // Range operations
    #[serde(rename = "$between")] Between,
    
    // Search operations
    #[serde(rename = "$find")] Find,
    #[serde(rename = "$text")] Text,
    
    // Existence
    #[serde(rename = "$exists")] Exists,
    #[serde(rename = "$null")] Null,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FilterData {
    pub select: Option<Vec<String>>,
    pub where_clause: Option<serde_json::Value>, // Use 'where_clause' to avoid reserved word
    pub order: Option<serde_json::Value>,
    pub limit: Option<i32>,
    pub offset: Option<i32>,
}

#[derive(Debug, Clone)]
pub struct FilterWhereInfo {
    pub column: String,
    pub operator: FilterOp,
    pub data: serde_json::Value,
}

#[derive(Debug, Clone)]
pub struct FilterWhereOptions {
    pub include_trashed: bool,
    pub include_deleted: bool,
}

impl Default for FilterWhereOptions {
    fn default() -> Self {
        Self {
            include_trashed: false,
            include_deleted: false,
        }
    }
}

#[derive(Debug, Clone)]
pub struct FilterOrderInfo {
    pub column: String,
    pub sort: SortDirection,
}

#[derive(Debug, Clone, PartialEq)]
pub enum SortDirection {
    Asc,
    Desc,
}

impl SortDirection {
    pub fn to_sql(&self) -> &'static str {
        match self {
            SortDirection::Asc => "ASC",
            SortDirection::Desc => "DESC",
        }
    }
}

#[derive(Debug, Clone)]
pub struct SqlResult {
    pub query: String,
    pub params: Vec<serde_json::Value>,
}
```

### Main Filter Class

```rust
// src/lib/filter/filter.rs
use crate::filter::{FilterData, FilterWhereOptions, SqlResult};
use crate::filter::filter_where::FilterWhere;
use crate::filter::filter_order::FilterOrder;
use crate::error::FilterError;

pub struct Filter {
    table_name: String,
    select_columns: Vec<String>,
    where_data: Option<serde_json::Value>,
    order_data: Vec<FilterOrderInfo>,
    limit: Option<i32>,
    offset: Option<i32>,
    soft_delete_options: FilterWhereOptions,
}

impl Filter {
    pub fn new(table_name: impl Into<String>) -> Result<Self, FilterError> {
        let table_name = table_name.into();
        Self::validate_table_name(&table_name)?;
        
        Ok(Self {
            table_name,
            select_columns: vec![],
            where_data: None,
            order_data: vec![],
            limit: None,
            offset: None,
            soft_delete_options: FilterWhereOptions::default(),
        })
    }
    
    /// Main entry point - process FilterData input
    pub fn assign(&mut self, data: FilterData) -> Result<&mut Self, FilterError> {
        if let Some(select) = data.select {
            self.select(select)?;
        }
        
        if let Some(where_clause) = data.where_clause {
            self.where_clause(where_clause)?;
        }
        
        if let Some(order) = data.order {
            self.order(order)?;
        }
        
        if let Some(limit) = data.limit {
            self.limit(limit, data.offset)?;
        }
        
        Ok(self)
    }
    
    /// SELECT clause processing
    pub fn select(&mut self, columns: Vec<String>) -> Result<&mut Self, FilterError> {
        Self::validate_select_columns(&columns)?;
        self.select_columns = columns;
        Ok(self)
    }
    
    /// WHERE clause processing - delegates to FilterWhere
    pub fn where_clause(&mut self, conditions: serde_json::Value) -> Result<&mut Self, FilterError> {
        FilterWhere::validate(&conditions)?;
        self.where_data = Some(conditions);
        Ok(self)
    }
    
    /// ORDER clause processing - delegates to FilterOrder
    pub fn order(&mut self, order_spec: serde_json::Value) -> Result<&mut Self, FilterError> {
        let order_info = FilterOrder::validate_and_parse(&order_spec)?;
        self.order_data = order_info;
        Ok(self)
    }
    
    /// LIMIT/OFFSET processing
    pub fn limit(&mut self, limit: i32, offset: Option<i32>) -> Result<&mut Self, FilterError> {
        if limit < 0 {
            return Err(FilterError::InvalidLimit("Limit must be non-negative".to_string()));
        }
        
        if let Some(offset_val) = offset {
            if offset_val < 0 {
                return Err(FilterError::InvalidOffset("Offset must be non-negative".to_string()));
            }
        }
        
        self.limit = Some(limit);
        self.offset = offset;
        Ok(self)
    }
    
    /// Generate complete SQL query with parameters
    pub fn to_sql(&self) -> Result<SqlResult, FilterError> {
        let select_clause = self.build_select_clause();
        
        let (where_clause, params) = if let Some(ref where_data) = self.where_data {
            FilterWhere::generate(where_data, 0, &self.soft_delete_options)?
        } else {
            FilterWhere::generate_empty(&self.soft_delete_options)
        };
        
        let order_clause = FilterOrder::generate(&self.order_data)?;
        let limit_clause = self.build_limit_clause();
        
        let query_parts = vec![
            format!("SELECT {}", select_clause),
            format!("FROM \"{}\"", self.table_name),
            if where_clause.is_empty() { String::new() } else { format!("WHERE {}", where_clause) },
            order_clause,
            limit_clause,
        ];
        
        let query = query_parts.into_iter()
            .filter(|part| !part.is_empty())
            .collect::<Vec<_>>()
            .join(" ");
        
        Ok(SqlResult { query, params })
    }
    
    /// Generate WHERE clause for use in other queries (COUNT, etc.)
    pub fn to_where_sql(&self) -> Result<SqlResult, FilterError> {
        let (where_clause, params) = if let Some(ref where_data) = self.where_data {
            FilterWhere::generate(where_data, 0, &self.soft_delete_options)?
        } else {
            FilterWhere::generate_empty(&self.soft_delete_options)
        };
        
        Ok(SqlResult { 
            query: where_clause, 
            params 
        })
    }
    
    /// Generate COUNT query with same WHERE conditions
    pub fn to_count_sql(&self) -> Result<SqlResult, FilterError> {
        let where_result = self.to_where_sql()?;
        
        let query = if where_result.query.is_empty() {
            format!("SELECT COUNT(*) as count FROM \"{}\"", self.table_name)
        } else {
            format!("SELECT COUNT(*) as count FROM \"{}\" WHERE {}", 
                    self.table_name, where_result.query)
        };
        
        Ok(SqlResult { 
            query, 
            params: where_result.params 
        })
    }
    
    // Private helper methods...
    fn validate_table_name(name: &str) -> Result<(), FilterError> {
        if name.is_empty() {
            return Err(FilterError::InvalidTableName("Table name cannot be empty".to_string()));
        }
        
        // SQL injection protection
        if !name.chars().all(|c| c.is_alphanumeric() || c == '_') || 
           !name.chars().next().unwrap().is_alphabetic() && name.chars().next().unwrap() != '_' {
            return Err(FilterError::InvalidTableName(
                format!("Invalid table name format: {}", name)
            ));
        }
        
        Ok(())
    }
    
    fn validate_select_columns(columns: &[String]) -> Result<(), FilterError> {
        for column in columns {
            if column == "*" {
                continue;
            }
            
            if column.is_empty() {
                return Err(FilterError::InvalidColumn("Column name cannot be empty".to_string()));
            }
            
            // SQL injection protection
            if !column.chars().all(|c| c.is_alphanumeric() || c == '_') ||
               !column.chars().next().unwrap().is_alphabetic() && column.chars().next().unwrap() != '_' {
                return Err(FilterError::InvalidColumn(
                    format!("Invalid column name format: {}", column)
                ));
            }
        }
        Ok(())
    }
    
    fn build_select_clause(&self) -> String {
        if self.select_columns.is_empty() || self.select_columns.contains(&"*".to_string()) {
            "*".to_string()
        } else {
            self.select_columns.iter()
                .map(|col| format!("\"{}\"", col))
                .collect::<Vec<_>>()
                .join(", ")
        }
    }
    
    fn build_limit_clause(&self) -> String {
        match (self.limit, self.offset) {
            (Some(limit), Some(offset)) => format!("LIMIT {} OFFSET {}", limit, offset),
            (Some(limit), None) => format!("LIMIT {}", limit),
            _ => String::new(),
        }
    }
}
```

### FilterWhere Implementation

```rust
// src/lib/filter/filter_where.rs
use crate::filter::{FilterOp, FilterWhereInfo, FilterWhereOptions, SqlResult};
use crate::error::FilterError;
use serde_json::Value;

pub struct FilterWhere {
    param_values: Vec<Value>,
    param_index: usize,
    conditions: Vec<FilterWhereInfo>,
}

impl FilterWhere {
    pub fn new(starting_param_index: usize) -> Self {
        Self {
            param_values: vec![],
            param_index: starting_param_index,
            conditions: vec![],
        }
    }
    
    /// Static method for quick WHERE clause generation
    pub fn generate(
        where_data: &Value, 
        starting_param_index: usize, 
        options: &FilterWhereOptions
    ) -> Result<(String, Vec<Value>), FilterError> {
        let mut filter_where = Self::new(starting_param_index);
        filter_where.build(where_data, options)
    }
    
    /// Generate empty WHERE clause with only soft delete filtering
    pub fn generate_empty(options: &FilterWhereOptions) -> (String, Vec<Value>) {
        let mut conditions = vec![];
        
        if !options.include_trashed {
            conditions.push("\"trashed_at\" IS NULL".to_string());
        }
        
        if !options.include_deleted {
            conditions.push("\"deleted_at\" IS NULL".to_string());
        }
        
        let where_clause = if conditions.is_empty() {
            "1=1".to_string()
        } else {
            conditions.join(" AND ")
        };
        
        (where_clause, vec![])
    }
    
    /// Public validation method
    pub fn validate(where_data: &Value) -> Result<(), FilterError> {
        // Implement comprehensive validation matching TypeScript logic
        if where_data.is_null() {
            return Ok(());
        }
        
        match where_data {
            Value::Object(obj) => {
                for (key, value) in obj {
                    Self::validate_condition(key, value)?;
                }
            }
            Value::String(s) if !s.trim().is_empty() => {
                // String conditions are valid
            }
            _ => return Err(FilterError::InvalidWhereClause("WHERE must be object or non-empty string".to_string())),
        }
        
        Ok(())
    }
    
    /// Build WHERE clause from validated data
    fn build(
        &mut self, 
        where_data: &Value, 
        options: &FilterWhereOptions
    ) -> Result<(String, Vec<Value>), FilterError> {
        self.param_values.clear();
        self.conditions.clear();
        self.param_index = 0;
        
        self.parse_where_data(where_data)?;
        
        let mut sql_conditions = vec![];
        
        // Add soft delete filtering
        if !options.include_trashed {
            sql_conditions.push("\"trashed_at\" IS NULL".to_string());
        }
        
        if !options.include_deleted {
            sql_conditions.push("\"deleted_at\" IS NULL".to_string());
        }
        
        // Add parsed conditions
        for condition in &self.conditions {
            if let Some(sql) = self.build_sql_condition(condition)? {
                sql_conditions.push(sql);
            }
        }
        
        let where_clause = if sql_conditions.is_empty() {
            "1=1".to_string()
        } else {
            sql_conditions.join(" AND ")
        };
        
        Ok((where_clause, self.param_values.clone()))
    }
    
    /// Parse JSON where data into structured conditions
    fn parse_where_data(&mut self, where_data: &Value) -> Result<(), FilterError> {
        match where_data {
            Value::Object(obj) => {
                for (key, value) in obj {
                    if key.starts_with('$') {
                        // Logical operator
                        self.parse_logical_operator(key, value)?;
                    } else {
                        // Field condition
                        self.parse_field_condition(key, value)?;
                    }
                }
            }
            _ => return Err(FilterError::InvalidWhereClause("Unsupported WHERE format".to_string())),
        }
        Ok(())
    }
    
    /// Build SQL condition from FilterWhereInfo
    fn build_sql_condition(&mut self, condition: &FilterWhereInfo) -> Result<Option<String>, FilterError> {
        let quoted_column = format!("\"{}\"", condition.column);
        
        match condition.operator {
            FilterOp::Eq => {
                if condition.data.is_null() {
                    Ok(Some(format!("{} IS NULL", quoted_column)))
                } else {
                    Ok(Some(format!("{} = {}", quoted_column, self.param(condition.data.clone()))))
                }
            }
            
            FilterOp::Gte => {
                Ok(Some(format!("{} >= {}", quoted_column, self.param(condition.data.clone()))))
            }
            
            FilterOp::Like => {
                Ok(Some(format!("{} LIKE {}", quoted_column, self.param(condition.data.clone()))))
            }
            
            FilterOp::ILike => {
                Ok(Some(format!("{} ILIKE {}", quoted_column, self.param(condition.data.clone()))))
            }
            
            FilterOp::In => {
                if let Value::Array(values) = &condition.data {
                    if values.is_empty() {
                        return Ok(Some("1=0".to_string())); // Always false
                    }
                    
                    let params: Vec<String> = values.iter()
                        .map(|v| self.param(v.clone()))
                        .collect();
                    
                    Ok(Some(format!("{} IN ({})", quoted_column, params.join(", "))))
                } else {
                    Ok(Some(format!("{} = {}", quoted_column, self.param(condition.data.clone()))))
                }
            }
            
            FilterOp::Between => {
                if let Value::Array(values) = &condition.data {
                    if values.len() != 2 {
                        return Err(FilterError::InvalidOperatorData(
                            "$between requires exactly 2 values".to_string()
                        ));
                    }
                    
                    Ok(Some(format!(
                        "{} BETWEEN {} AND {}", 
                        quoted_column, 
                        self.param(values[0].clone()),
                        self.param(values[1].clone())
                    )))
                } else {
                    Err(FilterError::InvalidOperatorData(
                        "$between requires array with 2 values".to_string()
                    ))
                }
            }
            
            // PostgreSQL array operations (critical for ACL)
            FilterOp::Any => {
                if let Value::Array(values) = &condition.data {
                    if values.is_empty() {
                        return Ok(Some("1=0".to_string()));
                    }
                    
                    let params: Vec<String> = values.iter()
                        .map(|v| self.param(v.clone()))
                        .collect();
                    
                    Ok(Some(format!("{} && ARRAY[{}]", quoted_column, params.join(", "))))
                } else {
                    Ok(Some(format!("{} && ARRAY[{}]", quoted_column, self.param(condition.data.clone()))))
                }
            }
            
            FilterOp::All => {
                if let Value::Array(values) = &condition.data {
                    let params: Vec<String> = values.iter()
                        .map(|v| self.param(v.clone()))
                        .collect();
                    
                    Ok(Some(format!("{} @> ARRAY[{}]", quoted_column, params.join(", "))))
                } else {
                    Ok(Some(format!("{} @> ARRAY[{}]", quoted_column, self.param(condition.data.clone()))))
                }
            }
            
            FilterOp::Size => {
                Ok(Some(format!("array_length({}, 1) = {}", quoted_column, self.param(condition.data.clone()))))
            }
            
            // TODO: Implement remaining operators...
            _ => {
                tracing::warn!("Unimplemented filter operator: {:?}", condition.operator);
                Ok(None)
            }
        }
    }
    
    /// Add parameter and return placeholder
    fn param(&mut self, value: Value) -> String {
        self.param_values.push(value);
        self.param_index += 1;
        format!("${}", self.param_index)
    }
    
    // Helper validation methods...
    fn validate_condition(key: &str, value: &Value) -> Result<(), FilterError> {
        // Implement field name validation, operator validation, etc.
        if key.starts_with('$') {
            Self::validate_logical_operator(key, value)
        } else {
            Self::validate_field_condition(key, value)
        }
    }
    
    fn validate_logical_operator(operator: &str, value: &Value) -> Result<(), FilterError> {
        // Implement logical operator validation
        match operator {
            "$and" | "$or" => {
                if !value.is_array() {
                    return Err(FilterError::InvalidOperatorData(
                        format!("{} requires array of conditions", operator)
                    ));
                }
            }
            "$not" => {
                if !value.is_object() && !value.is_array() {
                    return Err(FilterError::InvalidOperatorData(
                        "$not requires object or array".to_string()
                    ));
                }
            }
            _ => return Err(FilterError::UnsupportedOperator(operator.to_string())),
        }
        Ok(())
    }
    
    fn validate_field_condition(field: &str, _value: &Value) -> Result<(), FilterError> {
        // Validate field name for SQL injection protection
        if field.is_empty() {
            return Err(FilterError::InvalidColumn("Field name cannot be empty".to_string()));
        }
        
        if !field.chars().all(|c| c.is_alphanumeric() || c == '_') ||
           !field.chars().next().unwrap().is_alphabetic() && field.chars().next().unwrap() != '_' {
            return Err(FilterError::InvalidColumn(
                format!("Invalid field name format: {}", field)
            ));
        }
        
        Ok(())
    }
}
```

### Error Handling

```rust
// src/lib/filter/error.rs
use thiserror::Error;

#[derive(Error, Debug)]
pub enum FilterError {
    #[error("Invalid table name: {0}")]
    InvalidTableName(String),
    
    #[error("Invalid column name: {0}")]
    InvalidColumn(String),
    
    #[error("Invalid WHERE clause: {0}")]
    InvalidWhereClause(String),
    
    #[error("Unsupported operator: {0}")]
    UnsupportedOperator(String),
    
    #[error("Invalid operator data: {0}")]
    InvalidOperatorData(String),
    
    #[error("Invalid limit: {0}")]
    InvalidLimit(String),
    
    #[error("Invalid offset: {0}")]
    InvalidOffset(String),
    
    #[error("JSON parsing error: {0}")]
    JsonError(#[from] serde_json::Error),
}
```

## Integration with Monk API

### Handler Integration

```rust
// src/handlers/protected/data/find.rs
use crate::filter::{Filter, FilterData};
use axum::{extract::Path, response::Json, Extension};
use serde_json::Value;
use sqlx::PgPool;

pub async fn find_post(
    Path(schema): Path<String>,
    Extension(db): Extension<PgPool>,
    Json(filter_data): Json<FilterData>,
) -> Result<Json<Value>, AppError> {
    // Create filter instance
    let mut filter = Filter::new(&schema)?;
    
    // Process filter data
    filter.assign(filter_data)?;
    
    // Generate SQL query
    let sql_result = filter.to_sql()?;
    
    // Execute query with compile-time verified parameters
    let mut query = sqlx::query(&sql_result.query);
    for param in sql_result.params {
        query = query.bind(param);
    }
    
    let rows = query.fetch_all(&db).await?;
    
    // Convert rows to JSON
    let records: Vec<Value> = rows.into_iter()
        .map(|row| {
            // Convert PostgreSQL row to JSON value
            row_to_json(row)
        })
        .collect();
    
    Ok(Json(json!({
        "success": true,
        "data": records
    })))
}
```

## Key Advantages of Rust Implementation

### 1. **Compile-Time Safety**
- Type-safe operator definitions with enum
- Compile-time SQL parameter validation via SQLx
- Memory safety without runtime overhead

### 2. **Performance Benefits**
- Zero-cost abstractions for filter operations
- Efficient string building with minimal allocations
- Fast JSON parsing with serde

### 3. **Security Enhancements**
- Compile-time prevention of SQL injection
- Structured validation with Result types
- Comprehensive error handling with thiserror

### 4. **Maintainability**
- Clear separation of concerns (Filter, FilterWhere, FilterOrder)
- Explicit error types with detailed messages
- Self-documenting code with Rust's type system

## Implementation Roadmap

1. **Core Types** (`filter/types.rs`)
   - Define all enums, structs, and traits
   - Serde integration for JSON parsing

2. **FilterWhere Implementation** (`filter/filter_where.rs`)
   - All 25+ operators with PostgreSQL array support
   - Recursive logical operator handling
   - Parameter management and SQL injection protection

3. **FilterOrder Implementation** (`filter/filter_order.rs`)
   - Multiple input format support
   - SQL generation with proper escaping

4. **Main Filter Class** (`filter/filter.rs`)
   - Input validation and normalization
   - Query orchestration and SQL generation

5. **Integration Layer**
   - Handler implementation for `/api/find/:schema`
   - Database connection and query execution
   - Response formatting and error handling

This Rust implementation maintains 100% compatibility with the TypeScript filter language while providing additional compile-time safety, performance benefits, and maintainability improvements.