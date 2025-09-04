use sqlx::PgPool;
use serde::{Serialize, Deserialize};
use sha2::{Sha256, Digest};
use uuid::Uuid;
use anyhow::Result;
use chrono::{DateTime, Utc, NaiveDateTime, TimeZone};
use crate::database::manager::{DatabaseManager, DatabaseError};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TenantInfo {
    pub id: Uuid,
    pub name: String,
    pub database: String,
    pub host: Option<String>,
    pub is_active: Option<bool>,
    pub tenant_type: Option<String>,
    pub access_read: Option<Vec<Uuid>>,
    pub access_edit: Option<Vec<Uuid>>,
    pub access_full: Option<Vec<Uuid>>,
    pub access_deny: Option<Vec<Uuid>>,
    pub created_at: NaiveDateTime,
    pub updated_at: NaiveDateTime,
    pub trashed_at: Option<NaiveDateTime>,
    pub deleted_at: Option<NaiveDateTime>,
}

#[derive(Debug, thiserror::Error)]
pub enum TenantError {
    #[error("Database error: {0}")]
    Database(#[from] sqlx::Error),
    #[error("Database manager error: {0}")]
    DatabaseManager(#[from] DatabaseError),
    #[error("Tenant already exists: {0}")]
    AlreadyExists(String),
    #[error("Template not found: {0}")]
    TemplateNotFound(String),
    #[error("Invalid tenant name: {0}")]
    InvalidName(String),
}

pub struct TenantService {
    main_pool: PgPool,
}

impl TenantService {
    pub async fn new() -> Result<Self, TenantError> {
        let main_pool = DatabaseManager::main_pool().await?;
        Ok(Self {
            main_pool,
        })
    }

    /// Create a new tenant from a template
    pub async fn create_tenant(
        &self, 
        tenant_name: &str, 
        template: &str
    ) -> Result<TenantInfo, TenantError> {
        // Validate tenant name
        self.validate_tenant_name(tenant_name)?;
        
        // (a) Hash tenant name to database name
        let tenant_db = self.hash_tenant_name(tenant_name);
        let template_db = format!("template_{}", template);
        
        // Check if tenant already exists
        if self.tenant_exists(tenant_name).await? {
            return Err(TenantError::AlreadyExists(tenant_name.to_string()));
        }
        
        // Check if template exists
        if !self.template_exists(&template_db).await? {
            return Err(TenantError::TemplateNotFound(template.to_string()));
        }
        
        // (b) Clone template database to new tenant database
        DatabaseManager::clone_database(&template_db, &tenant_db).await?;
        
        // (c) Insert new row in main tenants table
        let tenant_info = self.register_tenant(tenant_name, &tenant_db, template).await?;
        
        Ok(tenant_info)
    }

    /// Hash tenant name to consistent database name
    fn hash_tenant_name(&self, name: &str) -> String {
        let mut hasher = Sha256::new();
        hasher.update(name.as_bytes());
        let hash = hasher.finalize();
        let hash_str = format!("{:x}", hash);
        
        // Use first 16 characters of hash for reasonable DB name length
        format!("tenant_{}", &hash_str[..16])
    }

    /// Validate tenant name follows rules
    fn validate_tenant_name(&self, name: &str) -> Result<(), TenantError> {
        if name.is_empty() || name.len() < 2 {
            return Err(TenantError::InvalidName("Tenant name must be at least 2 characters".to_string()));
        }
        
        if name.len() > 100 {
            return Err(TenantError::InvalidName("Tenant name must be less than 100 characters".to_string()));
        }
        
        // Only allow alphanumeric, hyphens, and underscores
        if !name.chars().all(|c| c.is_alphanumeric() || c == '-' || c == '_') {
            return Err(TenantError::InvalidName("Tenant name can only contain letters, numbers, hyphens, and underscores".to_string()));
        }
        
        Ok(())
    }

    /// Check if tenant already exists in registry
    async fn tenant_exists(&self, tenant_name: &str) -> Result<bool, TenantError> {
        let count: (i64,) = sqlx::query_as(
            "SELECT COUNT(*) FROM tenants WHERE name = $1 AND deleted_at IS NULL"
        )
        .bind(tenant_name)
        .fetch_one(&self.main_pool)
        .await?;
        
        Ok(count.0 > 0)
    }

    /// Check if template database exists
    async fn template_exists(&self, template_db: &str) -> Result<bool, TenantError> {
        let count: (i64,) = sqlx::query_as(
            "SELECT COUNT(*) FROM pg_database WHERE datname = $1"
        )
        .bind(template_db)
        .fetch_one(&self.main_pool)
        .await?;
        
        Ok(count.0 > 0)
    }

    /// Register tenant in main tenants table
    async fn register_tenant(
        &self, 
        tenant_name: &str, 
        tenant_db: &str, 
        template: &str
    ) -> Result<TenantInfo, TenantError> {
        let row = sqlx::query!(
            r#"
            INSERT INTO tenants (name, database, host, is_active, tenant_type)
            VALUES ($1, $2, 'localhost', true, 'normal')
            RETURNING 
                id,
                name,
                database,
                host,
                is_active,
                tenant_type,
                access_read,
                access_edit,
                access_full,
                access_deny,
                created_at,
                updated_at,
                trashed_at,
                deleted_at
            "#,
            tenant_name,
            tenant_db
        )
        .fetch_one(&self.main_pool)
        .await?;
        
        // Note: template is not stored in this schema, it's inferred from the database cloning operation
        let _ = template; // Acknowledge unused parameter
        
        Ok(TenantInfo {
            id: row.id,
            name: row.name,
            database: row.database,
            host: row.host,
            is_active: row.is_active,
            tenant_type: row.tenant_type,
            access_read: row.access_read,
            access_edit: row.access_edit,
            access_full: row.access_full,
            access_deny: row.access_deny,
            created_at: row.created_at,
            updated_at: row.updated_at,
            trashed_at: row.trashed_at,
            deleted_at: row.deleted_at,
        })
    }

    /// Get tenant info by name
    pub async fn get_tenant(&self, tenant_name: &str) -> Result<Option<TenantInfo>, TenantError> {
        let row = sqlx::query!(
            r#"
            SELECT id, name, database, host, is_active, tenant_type, 
                   access_read, access_edit, access_full, access_deny,
                   created_at, updated_at, trashed_at, deleted_at
            FROM tenants 
            WHERE name = $1 AND deleted_at IS NULL
            "#,
            tenant_name
        )
        .fetch_optional(&self.main_pool)
        .await?;
        
        Ok(row.map(|r| TenantInfo {
            id: r.id,
            name: r.name,
            database: r.database,
            host: r.host,
            is_active: r.is_active,
            tenant_type: r.tenant_type,
            access_read: r.access_read,
            access_edit: r.access_edit,
            access_full: r.access_full,
            access_deny: r.access_deny,
            created_at: r.created_at,
            updated_at: r.updated_at,
            trashed_at: r.trashed_at,
            deleted_at: r.deleted_at,
        }))
    }

    /// List all tenants
    pub async fn list_tenants(&self) -> Result<Vec<TenantInfo>, TenantError> {
        let rows = sqlx::query!(
            r#"
            SELECT id, name, database, host, is_active, tenant_type, 
                   access_read, access_edit, access_full, access_deny,
                   created_at, updated_at, trashed_at, deleted_at
            FROM tenants 
            WHERE deleted_at IS NULL
            ORDER BY created_at DESC
            "#
        )
        .fetch_all(&self.main_pool)
        .await?;
        
        let tenants = rows.into_iter().map(|r| TenantInfo {
            id: r.id,
            name: r.name,
            database: r.database,
            host: r.host,
            is_active: r.is_active,
            tenant_type: r.tenant_type,
            access_read: r.access_read,
            access_edit: r.access_edit,
            access_full: r.access_full,
            access_deny: r.access_deny,
            created_at: r.created_at,
            updated_at: r.updated_at,
            trashed_at: r.trashed_at,
            deleted_at: r.deleted_at,
        }).collect();
        
        Ok(tenants)
    }

    /// Get connection pool for specific tenant database
    pub async fn get_tenant_pool(&self, tenant_name: &str) -> Result<PgPool, TenantError> {
        let tenant = self.get_tenant(tenant_name).await?
            .ok_or_else(|| TenantError::InvalidName(format!("Tenant not found: {}", tenant_name)))?;
        
        let pool = DatabaseManager::tenant_pool(&tenant.database).await?;
        Ok(pool)
    }
}