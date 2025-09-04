use serde_json::Value;
use sqlx::{self, postgres::PgArguments, FromRow, PgPool, Row};

use crate::database::manager::DatabaseError;
use crate::filter::{Filter, FilterData};
use crate::filter::types::SqlResult;

pub struct QueryBuilder<T> {
    table_name: String,
    select_columns: Option<Vec<String>>, // reserved for future explicit select
    filter: Option<Filter>,
    _phantom: std::marker::PhantomData<T>,
}

impl<T> QueryBuilder<T>
where
    T: for<'r> FromRow<'r, sqlx::postgres::PgRow> + Send + Unpin,
{
    pub fn new(table_name: impl Into<String>) -> Result<Self, DatabaseError> {
        let name = table_name.into();
        // Reuse Filter table name validation
        crate::filter::filter::Filter::new(&name).map_err(|e| DatabaseError::QueryError(e.to_string()))?;
        Ok(Self {
            table_name: name,
            select_columns: None,
            filter: None,
            _phantom: std::marker::PhantomData,
        })
    }

    pub fn select(mut self, columns: Vec<String>) -> Self {
        self.select_columns = Some(columns);
        self
    }

    pub fn filter(mut self, filter_data: FilterData) -> Result<Self, DatabaseError> {
        let mut filter = Filter::new(&self.table_name).map_err(|e| DatabaseError::QueryError(e.to_string()))?;
        filter
            .assign(filter_data)
            .map_err(|e| DatabaseError::QueryError(e.to_string()))?;
        self.filter = Some(filter);
        Ok(self)
    }

    pub async fn select_all(self, pool: &PgPool) -> Result<Vec<T>, DatabaseError> {
        let sql_result = self.sql_result()?;
        let mut q = sqlx::query_as::<_, T>(&sql_result.query);
        for p in sql_result.params.iter() {
            q = bind_param_query_as(q, p);
        }
        let rows = q.fetch_all(pool).await?;
        Ok(rows)
    }

    pub async fn select_one(self, pool: &PgPool) -> Result<T, DatabaseError> {
        let sql_result = self.sql_result()?;
        let mut q = sqlx::query_as::<_, T>(&sql_result.query);
        for p in sql_result.params.iter() {
            q = bind_param_query_as(q, p);
        }
        let row = q.fetch_one(pool).await?;
        Ok(row)
    }

    pub async fn select_optional(self, pool: &PgPool) -> Result<Option<T>, DatabaseError> {
        let sql_result = self.sql_result()?;
        let mut q = sqlx::query_as::<_, T>(&sql_result.query);
        for p in sql_result.params.iter() {
            q = bind_param_query_as(q, p);
        }
        let row = q.fetch_optional(pool).await?;
        Ok(row)
    }

    pub async fn count(self, pool: &PgPool) -> Result<i64, DatabaseError> {
        let sql_result = if let Some(filter) = self.filter {
            filter.to_count_sql().map_err(|e| DatabaseError::QueryError(e.to_string()))?
        } else {
            SqlResult { query: format!("SELECT COUNT(*) as count FROM \"{}\"", self.table_name), params: vec![] }
        };

        let mut q = sqlx::query(&sql_result.query);
        for p in sql_result.params.iter() {
            q = bind_param_query(q, p);
        }
        let row = q.fetch_one(pool).await?;
        let count: i64 = row.try_get("count")?;
        Ok(count)
    }

    fn sql_result(&self) -> Result<SqlResult, DatabaseError> {
        if let Some(filter) = &self.filter {
            filter
                .to_sql()
                .map_err(|e| DatabaseError::QueryError(e.to_string()))
        } else {
            Ok(SqlResult { query: format!("SELECT * FROM \"{}\"", self.table_name), params: vec![] })
        }
    }
}

fn bind_param_query<'q>(
    q: sqlx::query::Query<'q, sqlx::Postgres, PgArguments>,
    v: &'q Value,
) -> sqlx::query::Query<'q, sqlx::Postgres, PgArguments> {
    match v {
        Value::Null => {
            let none: Option<String> = None;
            q.bind(none)
        }
        Value::Bool(b) => q.bind(*b),
        Value::Number(n) => {
            if let Some(i) = n.as_i64() {
                q.bind(i)
            } else if let Some(u) = n.as_u64() {
                // Postgres doesn't have u64; cast down if safe
                q.bind(u as i64)
            } else if let Some(f) = n.as_f64() {
                q.bind(f)
            } else {
                q.bind(n.to_string())
            }
        }
        Value::String(s) => q.bind(s),
        Value::Array(_arr) => {
            // Arrays are expanded by FilterWhere before binding; this case should be rare here
            q
        }
        Value::Object(_) => q.bind(v.clone()), // JSONB
    }
}

fn bind_param_query_as<'q, O>(
    q: sqlx::query::QueryAs<'q, sqlx::Postgres, O, PgArguments>,
    v: &'q Value,
) -> sqlx::query::QueryAs<'q, sqlx::Postgres, O, PgArguments>
where
    O: for<'r> FromRow<'r, sqlx::postgres::PgRow>,
{
    match v {
        Value::Null => {
            let none: Option<String> = None;
            q.bind(none)
        }
        Value::Bool(b) => q.bind(*b),
        Value::Number(n) => {
            if let Some(i) = n.as_i64() {
                q.bind(i)
            } else if let Some(u) = n.as_u64() {
                q.bind(u as i64)
            } else if let Some(f) = n.as_f64() {
                q.bind(f)
            } else {
                q.bind(n.to_string())
            }
        }
        Value::String(s) => q.bind(s),
        Value::Array(_arr) => q,
        Value::Object(_) => q.bind(v.clone()),
    }
}

