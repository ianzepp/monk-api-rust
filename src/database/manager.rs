use sqlx::{postgres::PgPoolOptions, PgPool};
use std::collections::HashMap;
use std::sync::Arc;
use thiserror::Error;
use tokio::sync::RwLock;
use tracing::info;

/// Errors from DatabaseManager
#[derive(Debug, Error)]
pub enum DatabaseError {
    #[error("Missing configuration: {0}")]
    ConfigMissing(&'static str),

    #[error("Invalid database URL")]
    InvalidDatabaseUrl,

    #[error("Invalid tenant database name: {0}")]
    InvalidTenantName(String),

    #[error("Not found: {0}")]
    NotFound(String),
    
    #[error("Query error: {0}")]
    QueryError(String),

    #[error(transparent)]
    Sqlx(#[from] sqlx::Error),
}

/// Centralized connection pool manager for system and tenant databases
pub struct DatabaseManager {
    pools: Arc<RwLock<HashMap<String, PgPool>>>,
}

impl DatabaseManager {
    fn instance() -> &'static DatabaseManager {
        use std::sync::OnceLock;
        static INSTANCE: OnceLock<DatabaseManager> = OnceLock::new();
        INSTANCE.get_or_init(|| DatabaseManager {
            pools: Arc::new(RwLock::new(HashMap::new())),
        })
    }

    /// Name of the system database. Currently fixed as "monk_main".
    /// Future work: make this configurable via env, e.g., MONK_SYSTEM_DB_NAME.
    const SYSTEM_DB_NAME: &'static str = "monk_main";

    /// Get main system database pool
    pub async fn main_pool() -> Result<PgPool, DatabaseError> {
        Self::instance().get_pool(Self::SYSTEM_DB_NAME).await
    }

    /// Get tenant database pool (validated name)
    pub async fn tenant_pool(database_name: &str) -> Result<PgPool, DatabaseError> {
        if !Self::is_valid_db_name(database_name) {
            return Err(DatabaseError::InvalidTenantName(database_name.to_string()));
        }
        Self::instance().get_pool(database_name).await
    }

    /// Get existing pool or create a new one lazily
    async fn get_pool(&self, database_name: &str) -> Result<PgPool, DatabaseError> {
        // Fast path: try read lock
        {
            let pools = self.pools.read().await;
            if let Some(pool) = pools.get(database_name) {
                return Ok(pool.clone());
            }
        }

        // Build connection string by swapping DB name in DATABASE_URL path
        let connection_string = Self::build_connection_string(database_name)?;

        // Create pool (could expose settings via env in future)
        let pool = PgPoolOptions::new().connect(&connection_string).await?;

        // Store in cache
        {
            let mut pools = self.pools.write().await;
            pools.insert(database_name.to_string(), pool.clone());
        }

        info!("Created database pool for: {}", database_name);
        Ok(pool)
    }

    fn build_connection_string(database_name: &str) -> Result<String, DatabaseError> {
        let base = std::env::var("DATABASE_URL")
            .map_err(|_| DatabaseError::ConfigMissing("DATABASE_URL"))?;

        let mut url = url::Url::parse(&base).map_err(|_| DatabaseError::InvalidDatabaseUrl)?;
        // Replace the path to the database name (ensure leading slash)
        url.set_path(&format!("/{}", database_name));
        Ok(url.into_string())
    }

    /// Pings the main pool to ensure connectivity
    pub async fn health_check() -> Result<(), DatabaseError> {
        let pool = Self::main_pool().await?;
        sqlx::query("SELECT 1").execute(&pool).await?;
        Ok(())
    }

    /// Clone a database (for template-based tenant creation)
    pub async fn clone_database(source_db: &str, target_db: &str) -> Result<(), DatabaseError> {
        if !Self::is_valid_db_name(source_db) {
            return Err(DatabaseError::InvalidTenantName(source_db.to_string()));
        }
        if !Self::is_valid_db_name(target_db) {
            return Err(DatabaseError::InvalidTenantName(target_db.to_string()));
        }

        // Connect to postgres database for administrative operations
        let admin_pool = Self::instance().get_admin_pool().await?;
        
        // Create new database from template
        let query = format!(
            "CREATE DATABASE {} WITH TEMPLATE {}",
            Self::quote_identifier(target_db),
            Self::quote_identifier(source_db)
        );
        
        sqlx::query(&query).execute(&admin_pool).await?;
        
        info!("Cloned database {} -> {}", source_db, target_db);
        Ok(())
    }

    /// Get administrative connection pool (connects to postgres database)
    async fn get_admin_pool(&self) -> Result<PgPool, DatabaseError> {
        self.get_pool("postgres").await
    }

    /// Get tenant pool - instance method for TenantService
    pub async fn get_tenant_pool(&self, database_name: &str) -> Result<PgPool, DatabaseError> {
        self.get_pool(database_name).await
    }

    /// Create a new DatabaseManager instance (for services that need non-static access)
    pub fn new() -> Self {
        Self {
            pools: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Quote SQL identifier to prevent injection
    fn quote_identifier(name: &str) -> String {
        format!("\"{}\"", name.replace("\"", "\"\""))
    }

    /// Close and remove all pools (e.g., on shutdown)
    pub async fn close_all() {
        let manager = Self::instance();
        let mut pools = manager.pools.write().await;
        for (name, pool) in pools.drain() {
            pool.close().await;
            info!("Closed database pool: {}", name);
        }
    }

    /// Validate database names to prevent injection. Accepts:
    /// - exact "monk_main"
    /// - exact "postgres" (for admin operations)
    /// - names starting with "tenant_" followed by [a-zA-Z0-9_]+
    /// - names starting with "template_" followed by [a-zA-Z0-9_]+
    fn is_valid_db_name(name: &str) -> bool {
        if name == Self::SYSTEM_DB_NAME || name == "postgres" {
            return true;
        }
        if name.starts_with("tenant_") || name.starts_with("template_") {
            return name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_');
        }
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validates_db_names() {
        assert!(DatabaseManager::is_valid_db_name("monk_main"));
        assert!(DatabaseManager::is_valid_db_name("tenant_123abc_DEF"));
        assert!(!DatabaseManager::is_valid_db_name("system"));
        assert!(!DatabaseManager::is_valid_db_name("tenant-123"));
        assert!(!DatabaseManager::is_valid_db_name("tenant_; DROP DATABASE"));
    }

    #[test]
    fn builds_connection_string_swaps_path() {
        std::env::set_var(
            "DATABASE_URL",
            "postgres://user:pass@localhost:5432/postgres?sslmode=disable",
        );
        let s = DatabaseManager::build_connection_string("tenant_abc").unwrap();
        assert!(s.starts_with("postgres://user:pass@localhost:5432/tenant_abc"));
        assert!(s.ends_with("sslmode=disable"));
    }
}
