use serde_json::json;
use serde::Serialize;
use sqlx::{self, postgres::PgRow, FromRow, PgPool};
use uuid::Uuid;

use crate::database::manager::DatabaseError;
use crate::database::query_builder::QueryBuilder;
use crate::filter::FilterData;

pub struct Repository<T> {
    table_name: String,
    pool: PgPool,
    _phantom: std::marker::PhantomData<T>,
}

impl<T> Repository<T>
where
    T: for<'r> FromRow<'r, PgRow> + Send + Unpin + Serialize,
{
    pub fn new(table_name: impl Into<String>, pool: PgPool) -> Self {
        Self {
            table_name: table_name.into(),
            pool,
            _phantom: std::marker::PhantomData,
        }
    }

    pub async fn select_any(&self, filter_data: FilterData) -> Result<Vec<T>, DatabaseError> {
        QueryBuilder::<T>::new(&self.table_name)?
            .filter(filter_data)?
            .select_all(&self.pool)
            .await
    }

    pub async fn select_one(&self, filter_data: FilterData) -> Result<Option<T>, DatabaseError> {
        QueryBuilder::<T>::new(&self.table_name)?
            .filter(filter_data)?
            .select_optional(&self.pool)
            .await
    }

    pub async fn select_404(&self, filter_data: FilterData) -> Result<T, DatabaseError> {
        match QueryBuilder::<T>::new(&self.table_name)?
            .filter(filter_data)?
            .select_one(&self.pool)
            .await
        {
            Ok(row) => Ok(row),
            Err(DatabaseError::Sqlx(sqlx::Error::RowNotFound)) => {
                Err(DatabaseError::NotFound("Record not found".to_string()))
            }
            Err(other) => Err(other),
        }
    }

    pub async fn count(&self, filter_data: FilterData) -> Result<i64, DatabaseError> {
        QueryBuilder::<T>::new(&self.table_name)?
            .filter(filter_data)?
            .count(&self.pool)
            .await
    }

    pub async fn select_ids(&self, ids: Vec<Uuid>) -> Result<Vec<T>, DatabaseError> {
        if ids.is_empty() {
            return Ok(vec![]);
        }
        let filter = FilterData {
            where_clause: Some(json!({ "id": { "$in": ids } })),
            ..Default::default()
        };
        self.select_any(filter).await
    }
}

