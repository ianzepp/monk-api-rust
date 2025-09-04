use serde_json::Value;

use super::error::FilterError;
use super::filter_order::FilterOrder;
use super::filter_where::FilterWhere;
use super::types::{FilterData, FilterOrderInfo, FilterWhereOptions, SqlResult};

pub struct Filter {
    table_name: String,
    select_columns: Vec<String>,
    where_data: Option<Value>,
    order_data: Vec<FilterOrderInfo>,
    limit: Option<i32>,
    offset: Option<i32>,
    options: FilterWhereOptions,
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
            options: FilterWhereOptions::default(),
        })
    }

    pub fn assign(&mut self, data: FilterData) -> Result<&mut Self, FilterError> {
        if let Some(select) = data.select { self.select(select)?; }
        if let Some(where_clause) = data.where_clause { self.where_clause(where_clause)?; }
        if let Some(order) = data.order { self.order(order)?; }
        if let Some(limit) = data.limit { self.limit(limit, data.offset)?; }
        Ok(self)
    }

    pub fn select(&mut self, columns: Vec<String>) -> Result<&mut Self, FilterError> {
        Self::validate_select_columns(&columns)?;
        self.select_columns = columns;
        Ok(self)
    }

    pub fn where_clause(&mut self, conditions: Value) -> Result<&mut Self, FilterError> {
        FilterWhere::validate(&conditions)?;
        self.where_data = Some(conditions);
        Ok(self)
    }

    pub fn order(&mut self, order_spec: Value) -> Result<&mut Self, FilterError> {
        let order_info = FilterOrder::validate_and_parse(&order_spec)?;
        self.order_data = order_info;
        Ok(self)
    }

    pub fn limit(&mut self, limit: i32, offset: Option<i32>) -> Result<&mut Self, FilterError> {
        if limit < 0 { return Err(FilterError::InvalidLimit("Limit must be non-negative".to_string())); }
        if let Some(off) = offset { if off < 0 { return Err(FilterError::InvalidOffset("Offset must be non-negative".to_string())); } }
        
        // Apply max limit from config
        let max_limit = crate::config::CONFIG.filter.max_limit.unwrap_or(i32::MAX);
        let applied_limit = if limit > max_limit {
            if crate::config::CONFIG.filter.debug_logging {
                tracing::warn!("Limit {} exceeds max {}, capping to max", limit, max_limit);
            }
            max_limit
        } else {
            limit
        };
        
        self.limit = Some(applied_limit);
        self.offset = offset;
        Ok(self)
    }

    pub fn to_sql(&self) -> Result<SqlResult, FilterError> {
        let select_clause = self.build_select_clause();
        let (where_clause, params) = if let Some(ref where_data) = self.where_data {
            FilterWhere::generate(where_data, 0, &self.options)?
        } else {
            FilterWhere::generate_empty(&self.options)
        };
        let order_clause = FilterOrder::generate(&self.order_data)?;
        let limit_clause = self.build_limit_clause();

        let query = [
            format!("SELECT {}", select_clause),
            format!("FROM \"{}\"", self.table_name),
            if where_clause.is_empty() { String::new() } else { format!("WHERE {}", where_clause) },
            order_clause,
            limit_clause,
        ].into_iter().filter(|s| !s.is_empty()).collect::<Vec<_>>().join(" ");

        Ok(SqlResult { query, params })
    }

    pub fn to_where_sql(&self) -> Result<SqlResult, FilterError> {
        let (where_clause, params) = if let Some(ref where_data) = self.where_data {
            FilterWhere::generate(where_data, 0, &self.options)?
        } else {
            FilterWhere::generate_empty(&self.options)
        };
        Ok(SqlResult { query: where_clause, params })
    }

    pub fn to_count_sql(&self) -> Result<SqlResult, FilterError> {
        let where_result = self.to_where_sql()?;
        let query = if where_result.query.is_empty() {
            format!("SELECT COUNT(*) as count FROM \"{}\"", self.table_name)
        } else {
            format!("SELECT COUNT(*) as count FROM \"{}\" WHERE {}", self.table_name, where_result.query)
        };
        Ok(SqlResult { query, params: where_result.params })
    }

    fn validate_table_name(name: &str) -> Result<(), FilterError> {
        if name.is_empty() { return Err(FilterError::InvalidTableName("Table name cannot be empty".to_string())); }
        if !name.chars().all(|c| c.is_alphanumeric() || c == '_') || (!name.chars().next().unwrap().is_alphabetic() && name.chars().next().unwrap() != '_') {
            return Err(FilterError::InvalidTableName(format!("Invalid table name format: {}", name)));
        }
        Ok(())
    }

    fn validate_select_columns(columns: &[String]) -> Result<(), FilterError> {
        for column in columns {
            if column == "*" { continue; }
            if column.is_empty() { return Err(FilterError::InvalidColumn("Column name cannot be empty".to_string())); }
            if !column.chars().all(|c| c.is_alphanumeric() || c == '_') || (!column.chars().next().unwrap().is_alphabetic() && column.chars().next().unwrap() != '_') {
                return Err(FilterError::InvalidColumn(format!("Invalid column name format: {}", column)));
            }
        }
        Ok(())
    }

    fn build_select_clause(&self) -> String {
        if self.select_columns.is_empty() || self.select_columns.contains(&"*".to_string()) {
            "*".to_string()
        } else {
            self.select_columns.iter().map(|c| format!("\"{}\"", c)).collect::<Vec<_>>().join(", ")
        }
    }

    fn build_limit_clause(&self) -> String {
        match (self.limit, self.offset) {
            (Some(l), Some(o)) => format!("LIMIT {} OFFSET {}", l, o),
            (Some(l), None) => format!("LIMIT {}", l),
            _ => String::new(),
        }
    }
}
