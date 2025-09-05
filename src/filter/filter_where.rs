use serde_json::Value;

use super::types::{FilterOp, FilterWhereInfo, FilterWhereOptions};
use super::error::FilterError;

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

    pub fn generate(where_data: &Value, starting_param_index: usize, options: &FilterWhereOptions) -> Result<(String, Vec<Value>), FilterError> {
        let mut filter_where = Self::new(starting_param_index);
        filter_where.build(where_data, options)
    }

    pub fn generate_empty(options: &FilterWhereOptions) -> (String, Vec<Value>) {
        let mut conditions = vec![];
        if !options.include_trashed { conditions.push("\"trashed_at\" IS NULL".to_string()); }
        if !options.include_deleted { conditions.push("\"deleted_at\" IS NULL".to_string()); }
        let where_clause = if conditions.is_empty() { "1=1".to_string() } else { conditions.join(" AND ") };
        (where_clause, vec![])
    }

    pub fn validate(where_data: &Value) -> Result<(), FilterError> {
        if where_data.is_null() { return Ok(()); }
        match where_data {
            Value::Object(_) | Value::String(_) => Ok(()),
            _ => Err(FilterError::InvalidWhereClause("WHERE must be object or string".to_string())),
        }
    }

    fn build(&mut self, where_data: &Value, options: &FilterWhereOptions) -> Result<(String, Vec<Value>), FilterError> {
        self.param_values.clear();
        self.conditions.clear();
        self.param_index = 0;

        self.parse_where_data(where_data)?;

        let mut sql_conditions = vec![];
        if !options.include_trashed { sql_conditions.push("\"trashed_at\" IS NULL".to_string()); }
        if !options.include_deleted { sql_conditions.push("\"deleted_at\" IS NULL".to_string()); }
        let conditions_snapshot = self.conditions.clone();
        for condition in &conditions_snapshot {
            if let Some(sql) = self.build_sql_condition(condition)? { sql_conditions.push(sql); }
        }
        let where_clause = if sql_conditions.is_empty() { "1=1".to_string() } else { sql_conditions.join(" AND ") };
        Ok((where_clause, self.param_values.clone()))
    }

    fn parse_where_data(&mut self, where_data: &Value) -> Result<(), FilterError> {
        match where_data {
            Value::Object(obj) => {
                for (key, value) in obj {
                    if key.starts_with('$') {
                        self.parse_logical_operator(key, value)?;
                    } else {
                        self.parse_field_condition(key, value)?;
                    }
                }
                Ok(())
            }
            Value::String(s) => {
                // Check if raw SQL is allowed in current environment
                if !crate::config::CONFIG.filter.allow_raw_sql {
                    return Err(FilterError::InvalidWhereClause(
                        "Raw SQL queries are disabled in this environment".to_string()
                    ));
                }
                // Log raw SQL usage for audit purposes
                if crate::config::CONFIG.security.enable_audit_logging {
                    tracing::warn!("Raw SQL query attempted: {}", s);
                }
                // Raw SQL predicate (use cautiously)
                self.conditions.push(FilterWhereInfo { column: s.clone(), operator: FilterOp::Text, data: Value::Null });
                Ok(())
            }
            _ => Err(FilterError::InvalidWhereClause("Unsupported WHERE format".to_string())),
        }
    }

    fn parse_logical_operator(&mut self, op: &str, value: &Value) -> Result<(), FilterError> {
        match op {
            "$and" | "$or" => {
                let arr = value.as_array().ok_or_else(|| FilterError::InvalidOperatorData(format!("{} requires array", op)))?;
                let mut sql_parts = Vec::new();
                for v in arr {
                    let (sql, params) = Self::generate(v, self.param_index, &FilterWhereOptions::default())?;
                    self.param_values.extend(params);
                    // Wrap subclause
                    sql_parts.push(format!("({})", sql));
                    self.param_index = self.param_values.len();
                }
                let joiner = if op == "$and" { " AND " } else { " OR " };
                let combined = sql_parts.join(joiner);
                // Store as a pseudo-condition using column="( … )"
                self.conditions.push(FilterWhereInfo { column: combined, operator: FilterOp::Text, data: Value::Null });
                Ok(())
            }
            "$not" => {
                let (sql, params) = Self::generate(value, self.param_index, &FilterWhereOptions::default())?;
                self.param_values.extend(params);
                self.param_index = self.param_values.len();
                self.conditions.push(FilterWhereInfo { column: format!("NOT ({})", sql), operator: FilterOp::Text, data: Value::Null });
                Ok(())
            }
            _ => Err(FilterError::UnsupportedOperator(op.to_string())),
        }
    }

    fn parse_field_condition(&mut self, field: &str, value: &Value) -> Result<(), FilterError> {
        if let Value::Object(obj) = value {
            for (op_key, op_val) in obj {
                let operator = Self::map_operator(op_key)?;
                self.conditions.push(FilterWhereInfo { column: field.to_string(), operator, data: op_val.clone() });
            }
        } else {
            // Implicit equality: { field: value }
            self.conditions.push(FilterWhereInfo { column: field.to_string(), operator: FilterOp::Eq, data: value.clone() });
        }
        Ok(())
    }

    fn map_operator(op_key: &str) -> Result<FilterOp, FilterError> {
        Ok(match op_key {
            "$eq" => FilterOp::Eq,
            "$ne" | "$neq" => FilterOp::Neq,
            "$gt" => FilterOp::Gt,
            "$gte" => FilterOp::Gte,
            "$lt" => FilterOp::Lt,
            "$lte" => FilterOp::Lte,
            "$like" => FilterOp::Like,
            "$ilike" => FilterOp::ILike,
            "$in" => FilterOp::In,
            "$between" => FilterOp::Between,
            "$any" => FilterOp::Any,
            "$all" => FilterOp::All,
            "$size" => FilterOp::Size,
            other => return Err(FilterError::UnsupportedOperator(other.to_string())),
        })
    }

    fn build_sql_condition(&mut self, condition: &FilterWhereInfo) -> Result<Option<String>, FilterError> {
        // Support pseudo conditions where column already contains SQL (for logical operators)
        if matches!(condition.operator, FilterOp::Text) && condition.data.is_null() {
            return Ok(Some(condition.column.clone()));
        }

        let quoted_column = format!("\"{}\"", condition.column);
        match condition.operator {
            FilterOp::Eq => {
                if condition.data.is_null() { Ok(Some(format!("{} IS NULL", quoted_column))) }
                else { Ok(Some(format!("{} = {}", quoted_column, self.param(condition.data.clone())))) }
            }
            FilterOp::Ne | FilterOp::Neq => {
                if condition.data.is_null() { Ok(Some(format!("{} IS NOT NULL", quoted_column))) }
                else { Ok(Some(format!("{} <> {}", quoted_column, self.param(condition.data.clone())))) }
            }
            FilterOp::Gt => Ok(Some(format!("{} > {}", quoted_column, self.param(condition.data.clone())))),
            FilterOp::Gte => Ok(Some(format!("{} >= {}", quoted_column, self.param(condition.data.clone())))),
            FilterOp::Lt => Ok(Some(format!("{} < {}", quoted_column, self.param(condition.data.clone())))),
            FilterOp::Lte => Ok(Some(format!("{} <= {}", quoted_column, self.param(condition.data.clone())))),
            FilterOp::Like => Ok(Some(format!("{} LIKE {}", quoted_column, self.param(condition.data.clone())))),
            FilterOp::ILike => Ok(Some(format!("{} ILIKE {}", quoted_column, self.param(condition.data.clone())))),
            FilterOp::In => {
                if let Value::Array(values) = &condition.data {
                    if values.is_empty() { return Ok(Some("1=0".to_string())); }
                    let params: Vec<String> = values.iter().map(|v| self.param(v.clone())).collect();
                    Ok(Some(format!("{} IN ({})", quoted_column, params.join(", "))))
                } else {
                    Ok(Some(format!("{} = {}", quoted_column, self.param(condition.data.clone()))))
                }
            }
            FilterOp::Between => {
                if let Value::Array(values) = &condition.data {
                    if values.len() != 2 { return Err(FilterError::InvalidOperatorData("$between requires exactly 2 values".to_string())); }
                    Ok(Some(format!("{} BETWEEN {} AND {}", quoted_column, self.param(values[0].clone()), self.param(values[1].clone()))))
                } else { Err(FilterError::InvalidOperatorData("$between requires array with 2 values".to_string())) }
            }
            FilterOp::Any => {
                if let Value::Array(values) = &condition.data {
                    if values.is_empty() { return Ok(Some("1=0".to_string())); }
                    let params: Vec<String> = values.iter().map(|v| self.param(v.clone())).collect();
                    Ok(Some(format!("{} && ARRAY[{}]", quoted_column, params.join(", "))))
                } else {
                    Ok(Some(format!("{} && ARRAY[{}]", quoted_column, self.param(condition.data.clone()))))
                }
            }
            FilterOp::All => {
                if let Value::Array(values) = &condition.data {
                    let params: Vec<String> = values.iter().map(|v| self.param(v.clone())).collect();
                    Ok(Some(format!("{} @> ARRAY[{}]", quoted_column, params.join(", "))))
                } else {
                    Ok(Some(format!("{} @> ARRAY[{}]", quoted_column, self.param(condition.data.clone()))))
                }
            }
            FilterOp::Size => Ok(Some(format!("array_length({}, 1) = {}", quoted_column, self.param(condition.data.clone())))),
            _ => Ok(None),
        }
    }

    fn param(&mut self, value: Value) -> String {
        self.param_values.push(value);
        self.param_index += 1;
        format!("${}", self.param_index)
    }
}
