use sqlx::PgPool;

use crate::database::manager::{DatabaseError, DatabaseManager};
use crate::database::models::tenant::Tenant;
use crate::database::models::user::User;

/// Check if a tenant exists in the main database by name
pub async fn find_tenant_by_name(tenant_name: &str) -> Result<Option<Tenant>, DatabaseError> {
    let pool = DatabaseManager::main_pool().await?;
    
    let tenant = sqlx::query_as::<_, Tenant>(
        "SELECT id, name, database, created_at, updated_at, trashed_at, deleted_at 
         FROM tenants 
         WHERE name = $1"
    )
    .bind(tenant_name)
    .fetch_optional(&pool)
    .await?;
    
    Ok(tenant)
}

/// Check if a user exists in the tenant database by auth (username)
pub async fn find_user_by_auth(tenant_db: &str, user_auth: &str) -> Result<Option<User>, DatabaseError> {
    let pool = DatabaseManager::tenant_pool(tenant_db).await?;
    
    let user = sqlx::query_as::<_, User>(
        "SELECT id, name, auth, access, access_read, access_edit, access_full, access_deny,
         created_at, updated_at, trashed_at, deleted_at
         FROM users 
         WHERE auth = $1"
    )
    .bind(user_auth)
    .fetch_optional(&pool)
    .await?;
    
    Ok(user)
}
