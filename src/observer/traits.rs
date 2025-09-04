use async_trait::async_trait;
use std::time::Duration;
use serde::{Serialize, Deserialize};
use crate::observer::error::ObserverError;
use crate::observer::context::ObserverContext;

/// Observer rings with semantic meaning - synchronous (0-6) and asynchronous (7-9)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum ObserverRing {
    DataPreparation = 0,    // Load existing data, merge updates
    InputValidation = 1,    // Schema validation, required fields
    Security = 2,           // Access control, soft delete protection
    Business = 3,           // Domain rules, workflows
    Enrichment = 4,         // Computed fields, defaults
    Database = 5,           // SQL execution ring (handled by repository)
    PostDatabase = 6,       // Immediate processing after database operations
    Audit = 7,              // Change tracking, compliance logging (async)
    Integration = 8,        // External APIs, webhooks (async)
    Notification = 9,       // User notifications, real-time updates (async)
}

impl ObserverRing {
    /// Check if ring executes synchronously (blocking)
    pub fn is_synchronous(&self) -> bool {
        (*self as u8) <= 6
    }
    
    /// Check if ring executes asynchronously (non-blocking)
    pub fn is_asynchronous(&self) -> bool {
        (*self as u8) >= 7
    }
    
    /// Get all rings for an operation type
    pub fn for_operation(operation: &Operation) -> Vec<Self> {
        use ObserverRing::*;
        
        match operation {
            Operation::Select => vec![
                DataPreparation, InputValidation, Security, Database, 
                PostDatabase, Integration, Notification
            ],
            Operation::Create | Operation::Update | Operation::Delete | Operation::Revert => vec![
                DataPreparation, InputValidation, Security, Business, 
                Enrichment, Database, PostDatabase, Audit, 
                Integration, Notification
            ],
        }
    }
}

/// Database operations supported by the observer system
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Operation {
    Create,
    Update, 
    Delete,
    Revert,
    Select,
}

/// Base trait for all observers with metadata and applicability checks
pub trait Observer: Send + Sync {
    /// Observer name for logging and debugging
    fn name(&self) -> &'static str;
    
    /// Which ring this observer belongs to
    fn ring(&self) -> ObserverRing;
    
    /// Check if observer applies to this operation
    fn applies_to_operation(&self, op: Operation) -> bool;
    
    /// Check if observer applies to this schema
    fn applies_to_schema(&self, schema: &str) -> bool;
    
    /// Execution timeout (default 5 seconds)
    fn timeout(&self) -> Duration { 
        Duration::from_secs(5) 
    }
    
    /// Priority within ring (lower numbers execute first)
    fn priority(&self) -> u8 { 
        50 
    }
}

/// Ring 0: Data Preparation - load existing data, merge updates
#[async_trait]
pub trait DataPreparationObserver: Observer {
    async fn execute(&self, ctx: &mut ObserverContext) -> Result<(), ObserverError>;
}

/// Ring 1: Input Validation - schema validation, required fields
#[async_trait]
pub trait InputValidationObserver: Observer {
    async fn execute(&self, ctx: &mut ObserverContext) -> Result<(), ObserverError>;
}

/// Ring 2: Security - access control, soft delete protection
#[async_trait]  
pub trait SecurityObserver: Observer {
    async fn execute(&self, ctx: &mut ObserverContext) -> Result<(), ObserverError>;
}

/// Ring 3: Business Logic - domain rules, workflows
#[async_trait]
pub trait BusinessObserver: Observer {
    async fn execute(&self, ctx: &mut ObserverContext) -> Result<(), ObserverError>;
}

/// Ring 4: Enrichment - computed fields, defaults
#[async_trait]
pub trait EnrichmentObserver: Observer {
    async fn execute(&self, ctx: &mut ObserverContext) -> Result<(), ObserverError>;
}

/// Ring 5: Database - SQL execution (handled by repository layer)
#[async_trait]
pub trait DatabaseObserver: Observer {
    async fn execute(&self, ctx: &mut ObserverContext) -> Result<(), ObserverError>;
}

/// Ring 6: Post-Database - immediate processing after database operations
#[async_trait]
pub trait PostDatabaseObserver: Observer {
    async fn execute(&self, ctx: &mut ObserverContext) -> Result<(), ObserverError>;
}

/// Ring 7: Audit - change tracking, compliance logging (async)
#[async_trait]
pub trait AuditObserver: Observer {
    async fn execute(&self, ctx: &ObserverContext) -> Result<(), ObserverError>;
}

/// Ring 8: Integration - external APIs, webhooks (async)
#[async_trait]
pub trait IntegrationObserver: Observer {
    async fn execute(&self, ctx: &ObserverContext) -> Result<(), ObserverError>;
}

/// Ring 9: Notification - user notifications, real-time updates (async)
#[async_trait]
pub trait NotificationObserver: Observer {
    async fn execute(&self, ctx: &ObserverContext) -> Result<(), ObserverError>;
}

/// Simplified approach: concrete observer types for dynamic dispatch
/// This avoids the trait object complexity while maintaining type safety
pub enum ObserverBox {
    DataPreparation(Box<dyn DataPreparationObserver>),
    InputValidation(Box<dyn InputValidationObserver>), 
    Security(Box<dyn SecurityObserver>),
    Business(Box<dyn BusinessObserver>),
    Enrichment(Box<dyn EnrichmentObserver>),
    Database(Box<dyn DatabaseObserver>),
    PostDatabase(Box<dyn PostDatabaseObserver>),
    Audit(Box<dyn AuditObserver>),
    Integration(Box<dyn IntegrationObserver>),
    Notification(Box<dyn NotificationObserver>),
}

impl ObserverBox {
    pub fn name(&self) -> &'static str {
        match self {
            ObserverBox::DataPreparation(o) => o.name(),
            ObserverBox::InputValidation(o) => o.name(), 
            ObserverBox::Security(o) => o.name(),
            ObserverBox::Business(o) => o.name(),
            ObserverBox::Enrichment(o) => o.name(),
            ObserverBox::Database(o) => o.name(),
            ObserverBox::PostDatabase(o) => o.name(),
            ObserverBox::Audit(o) => o.name(),
            ObserverBox::Integration(o) => o.name(),
            ObserverBox::Notification(o) => o.name(),
        }
    }
    
    pub fn ring(&self) -> ObserverRing {
        match self {
            ObserverBox::DataPreparation(o) => o.ring(),
            ObserverBox::InputValidation(o) => o.ring(), 
            ObserverBox::Security(o) => o.ring(),
            ObserverBox::Business(o) => o.ring(),
            ObserverBox::Enrichment(o) => o.ring(),
            ObserverBox::Database(o) => o.ring(),
            ObserverBox::PostDatabase(o) => o.ring(),
            ObserverBox::Audit(o) => o.ring(),
            ObserverBox::Integration(o) => o.ring(),
            ObserverBox::Notification(o) => o.ring(),
        }
    }
    
    pub fn applies_to_operation(&self, op: Operation) -> bool {
        match self {
            ObserverBox::DataPreparation(o) => o.applies_to_operation(op),
            ObserverBox::InputValidation(o) => o.applies_to_operation(op), 
            ObserverBox::Security(o) => o.applies_to_operation(op),
            ObserverBox::Business(o) => o.applies_to_operation(op),
            ObserverBox::Enrichment(o) => o.applies_to_operation(op),
            ObserverBox::Database(o) => o.applies_to_operation(op),
            ObserverBox::PostDatabase(o) => o.applies_to_operation(op),
            ObserverBox::Audit(o) => o.applies_to_operation(op),
            ObserverBox::Integration(o) => o.applies_to_operation(op),
            ObserverBox::Notification(o) => o.applies_to_operation(op),
        }
    }
    
    pub fn applies_to_schema(&self, schema: &str) -> bool {
        match self {
            ObserverBox::DataPreparation(o) => o.applies_to_schema(schema),
            ObserverBox::InputValidation(o) => o.applies_to_schema(schema), 
            ObserverBox::Security(o) => o.applies_to_schema(schema),
            ObserverBox::Business(o) => o.applies_to_schema(schema),
            ObserverBox::Enrichment(o) => o.applies_to_schema(schema),
            ObserverBox::Database(o) => o.applies_to_schema(schema),
            ObserverBox::PostDatabase(o) => o.applies_to_schema(schema),
            ObserverBox::Audit(o) => o.applies_to_schema(schema),
            ObserverBox::Integration(o) => o.applies_to_schema(schema),
            ObserverBox::Notification(o) => o.applies_to_schema(schema),
        }
    }
    
    pub fn timeout(&self) -> Duration {
        match self {
            ObserverBox::DataPreparation(o) => o.timeout(),
            ObserverBox::InputValidation(o) => o.timeout(), 
            ObserverBox::Security(o) => o.timeout(),
            ObserverBox::Business(o) => o.timeout(),
            ObserverBox::Enrichment(o) => o.timeout(),
            ObserverBox::Database(o) => o.timeout(),
            ObserverBox::PostDatabase(o) => o.timeout(),
            ObserverBox::Audit(o) => o.timeout(),
            ObserverBox::Integration(o) => o.timeout(),
            ObserverBox::Notification(o) => o.timeout(),
        }
    }
    
    pub async fn execute_sync(&self, ctx: &mut ObserverContext) -> Result<(), ObserverError> {
        match self {
            ObserverBox::DataPreparation(o) => o.execute(ctx).await,
            ObserverBox::InputValidation(o) => o.execute(ctx).await, 
            ObserverBox::Security(o) => o.execute(ctx).await,
            ObserverBox::Business(o) => o.execute(ctx).await,
            ObserverBox::Enrichment(o) => o.execute(ctx).await,
            ObserverBox::Database(o) => o.execute(ctx).await,
            ObserverBox::PostDatabase(o) => o.execute(ctx).await,
            _ => Ok(()), // Async observers don't execute in sync phase
        }
    }
    
    pub async fn execute_async(&self, ctx: &ObserverContext) -> Result<(), ObserverError> {
        match self {
            ObserverBox::Audit(o) => o.execute(ctx).await,
            ObserverBox::Integration(o) => o.execute(ctx).await,
            ObserverBox::Notification(o) => o.execute(ctx).await,
            _ => Ok(()), // Sync observers don't execute in async phase
        }
    }
}