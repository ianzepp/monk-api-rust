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
