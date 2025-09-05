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

// Operation enum moved to crate::types for shared usage
pub use crate::types::Operation;

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
pub trait Ring0: Observer {
    async fn execute(&self, ctx: &mut ObserverContext) -> Result<(), ObserverError>;
}

/// Ring 1: Input Validation - schema validation, required fields
#[async_trait]
pub trait Ring1: Observer {
    async fn execute(&self, ctx: &mut ObserverContext) -> Result<(), ObserverError>;
}

/// Ring 2: Security - access control, soft delete protection
#[async_trait]  
pub trait Ring2: Observer {
    async fn execute(&self, ctx: &mut ObserverContext) -> Result<(), ObserverError>;
}

/// Ring 3: Business Logic - domain rules, workflows
#[async_trait]
pub trait Ring3: Observer {
    async fn execute(&self, ctx: &mut ObserverContext) -> Result<(), ObserverError>;
}

/// Ring 4: Enrichment - computed fields, defaults
#[async_trait]
pub trait Ring4: Observer {
    async fn execute(&self, ctx: &mut ObserverContext) -> Result<(), ObserverError>;
}

/// Ring 5: Database - SQL execution (handled by repository layer)
#[async_trait]
pub trait Ring5: Observer {
    async fn execute(&self, ctx: &mut ObserverContext) -> Result<(), ObserverError>;
}

/// Ring 6: Post-Database - immediate processing after database operations
#[async_trait]
pub trait Ring6: Observer {
    async fn execute(&self, ctx: &mut ObserverContext) -> Result<(), ObserverError>;
}

/// Ring 7: Audit - change tracking, compliance logging (async)
#[async_trait]
pub trait Ring7: Observer {
    async fn execute(&self, ctx: &ObserverContext) -> Result<(), ObserverError>;
}

/// Ring 8: Integration - external APIs, webhooks (async)
#[async_trait]
pub trait Ring8: Observer {
    async fn execute(&self, ctx: &ObserverContext) -> Result<(), ObserverError>;
}

/// Ring 9: Notification - user notifications, real-time updates (async)
#[async_trait]
pub trait Ring9: Observer {
    async fn execute(&self, ctx: &ObserverContext) -> Result<(), ObserverError>;
}

/// Simplified approach: concrete observer types for dynamic dispatch
/// This avoids the trait object complexity while maintaining type safety
pub enum ObserverBox {
    Ring0(Box<dyn Ring0>),
    Ring1(Box<dyn Ring1>), 
    Ring2(Box<dyn Ring2>),
    Ring3(Box<dyn Ring3>),
    Ring4(Box<dyn Ring4>),
    Ring5(Box<dyn Ring5>),
    Ring6(Box<dyn Ring6>),
    Ring7(Box<dyn Ring7>),
    Ring8(Box<dyn Ring8>),
    Ring9(Box<dyn Ring9>),
}

impl ObserverBox {
    pub fn name(&self) -> &'static str {
        match self {
            ObserverBox::Ring0(o) => o.name(),
            ObserverBox::Ring1(o) => o.name(), 
            ObserverBox::Ring2(o) => o.name(),
            ObserverBox::Ring3(o) => o.name(),
            ObserverBox::Ring4(o) => o.name(),
            ObserverBox::Ring5(o) => o.name(),
            ObserverBox::Ring6(o) => o.name(),
            ObserverBox::Ring7(o) => o.name(),
            ObserverBox::Ring8(o) => o.name(),
            ObserverBox::Ring9(o) => o.name(),
        }
    }
    
    pub fn ring(&self) -> ObserverRing {
        match self {
            ObserverBox::Ring0(o) => o.ring(),
            ObserverBox::Ring1(o) => o.ring(), 
            ObserverBox::Ring2(o) => o.ring(),
            ObserverBox::Ring3(o) => o.ring(),
            ObserverBox::Ring4(o) => o.ring(),
            ObserverBox::Ring5(o) => o.ring(),
            ObserverBox::Ring6(o) => o.ring(),
            ObserverBox::Ring7(o) => o.ring(),
            ObserverBox::Ring8(o) => o.ring(),
            ObserverBox::Ring9(o) => o.ring(),
        }
    }
    
    pub fn applies_to_operation(&self, op: Operation) -> bool {
        match self {
            ObserverBox::Ring0(o) => o.applies_to_operation(op),
            ObserverBox::Ring1(o) => o.applies_to_operation(op), 
            ObserverBox::Ring2(o) => o.applies_to_operation(op),
            ObserverBox::Ring3(o) => o.applies_to_operation(op),
            ObserverBox::Ring4(o) => o.applies_to_operation(op),
            ObserverBox::Ring5(o) => o.applies_to_operation(op),
            ObserverBox::Ring6(o) => o.applies_to_operation(op),
            ObserverBox::Ring7(o) => o.applies_to_operation(op),
            ObserverBox::Ring8(o) => o.applies_to_operation(op),
            ObserverBox::Ring9(o) => o.applies_to_operation(op),
        }
    }
    
    pub fn applies_to_schema(&self, schema: &str) -> bool {
        match self {
            ObserverBox::Ring0(o) => o.applies_to_schema(schema),
            ObserverBox::Ring1(o) => o.applies_to_schema(schema), 
            ObserverBox::Ring2(o) => o.applies_to_schema(schema),
            ObserverBox::Ring3(o) => o.applies_to_schema(schema),
            ObserverBox::Ring4(o) => o.applies_to_schema(schema),
            ObserverBox::Ring5(o) => o.applies_to_schema(schema),
            ObserverBox::Ring6(o) => o.applies_to_schema(schema),
            ObserverBox::Ring7(o) => o.applies_to_schema(schema),
            ObserverBox::Ring8(o) => o.applies_to_schema(schema),
            ObserverBox::Ring9(o) => o.applies_to_schema(schema),
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
            ObserverBox::Ring0(o) => o.execute(ctx).await,
            ObserverBox::Ring1(o) => o.execute(ctx).await, 
            ObserverBox::Ring2(o) => o.execute(ctx).await,
            ObserverBox::Ring3(o) => o.execute(ctx).await,
            ObserverBox::Ring4(o) => o.execute(ctx).await,
            ObserverBox::Ring5(o) => o.execute(ctx).await,
            ObserverBox::Ring6(o) => o.execute(ctx).await,
            _ => Ok(()), // Async observers don't execute in sync phase
        }
    }
    
    pub async fn execute_async(&self, ctx: &ObserverContext) -> Result<(), ObserverError> {
        match self {
            ObserverBox::Ring7(o) => o.execute(ctx).await,
            ObserverBox::Ring8(o) => o.execute(ctx).await,
            ObserverBox::Ring9(o) => o.execute(ctx).await,
            _ => Ok(()), // Sync observers don't execute in async phase
        }
    }
}