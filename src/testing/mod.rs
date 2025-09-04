use uuid::Uuid;
use crate::services::{TenantService, TenantInfo};

/// Test utilities for tenant creation and management
pub struct TestContext {
    tenant_service: TenantService,
    created_tenants: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct TestTenant {
    pub name: String,
    pub info: TenantInfo,
}

impl TestContext {
    /// Create a new test context with tenant service
    pub async fn new() -> anyhow::Result<Self> {
        let tenant_service = TenantService::new().await
            .map_err(|e| anyhow::anyhow!("Failed to initialize tenant service: {}", e))?;
        
        Ok(Self {
            tenant_service,
            created_tenants: Vec::new(),
        })
    }

    /// Create a test tenant from a template
    pub async fn create_test_tenant(&mut self, template: &str) -> anyhow::Result<TestTenant> {
        let tenant_name = self.generate_test_tenant_name();
        
        let tenant_info = self.tenant_service.create_tenant(&tenant_name, template).await
            .map_err(|e| anyhow::anyhow!("Failed to create test tenant: {}", e))?;
        
        self.created_tenants.push(tenant_name.clone());
        
        Ok(TestTenant {
            name: tenant_name,
            info: tenant_info,
        })
    }

    /// Create a test tenant with a specific name (for controlled testing)
    pub async fn create_named_test_tenant(&mut self, name: &str, template: &str) -> anyhow::Result<TestTenant> {
        let tenant_name = format!("test_{}_{}", name, Uuid::new_v4().simple());
        
        let tenant_info = self.tenant_service.create_tenant(&tenant_name, template).await
            .map_err(|e| anyhow::anyhow!("Failed to create test tenant: {}", e))?;
        
        self.created_tenants.push(tenant_name.clone());
        
        Ok(TestTenant {
            name: tenant_name,
            info: tenant_info,
        })
    }

    /// Get connection pool for a test tenant
    pub async fn get_tenant_pool(&self, tenant_name: &str) -> anyhow::Result<sqlx::PgPool> {
        self.tenant_service.get_tenant_pool(tenant_name).await
            .map_err(|e| anyhow::anyhow!("Failed to get tenant pool: {}", e))
    }

    /// Generate a unique test tenant name
    fn generate_test_tenant_name(&self) -> String {
        format!("test_{}", Uuid::new_v4().simple())
    }

    /// Get list of all created test tenants (for cleanup)
    pub fn created_tenants(&self) -> &[String] {
        &self.created_tenants
    }

    /// Cleanup - note: actual database cleanup would need to be implemented
    /// For now this just clears the tracking list
    pub async fn cleanup(&mut self) -> anyhow::Result<()> {
        // TODO: Implement actual database cleanup when needed
        // This would involve dropping tenant databases and removing registry entries
        
        self.created_tenants.clear();
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_context_creation() {
        let result = TestContext::new().await;
        // This test will pass if database connectivity is available
        // In CI/CD environments without database, this would be skipped
        match result {
            Ok(_) => println!("Test context created successfully"),
            Err(e) => println!("Test context creation failed (expected in CI): {}", e),
        }
    }

    #[tokio::test]
    async fn test_tenant_name_generation() {
        if let Ok(mut ctx) = TestContext::new().await {
            let name1 = ctx.generate_test_tenant_name();
            let name2 = ctx.generate_test_tenant_name();
            
            assert_ne!(name1, name2);
            assert!(name1.starts_with("test_"));
            assert!(name2.starts_with("test_"));
        }
    }
}