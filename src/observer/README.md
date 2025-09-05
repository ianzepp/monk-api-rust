# Observer Pipeline System

The Observer Pipeline is a sophisticated data processing architecture that provides a flexible, extensible way to handle database operations through a series of execution rings. Each ring represents a specific phase in the data processing lifecycle, ensuring proper separation of concerns and execution order.

## Architecture Overview

The pipeline processes operations through **10 execution rings (0-9)**, where:
- **Rings 0-6**: Execute synchronously (blocking) in sequence
- **Rings 7-9**: Execute asynchronously (non-blocking) after sync completion

```
Operation Request
       ↓
   Ring 0: Data Preparation
       ↓
   Ring 1: Input Validation  
       ↓
   Ring 2: Security
       ↓
   Ring 3: Business Logic
       ↓
   Ring 4: Enrichment
       ↓
   Ring 5: Database
       ↓
   Ring 6: Post-Database
       ↓
   ┌──────────────────┐
   │  Async Phase     │
   │  Ring 7: Audit   │ 
   │  Ring 8: Integration │
   │  Ring 9: Notification │
   └──────────────────┘
```

## Execution Rings

### Ring 0: Data Preparation
**Purpose**: Load existing data, merge updates  
**Execution**: Synchronous  
**Use Cases**: Load existing records, merge partial updates, normalize input data

### Ring 1: Input Validation
**Purpose**: Schema validation, required fields  
**Execution**: Synchronous  
**Use Cases**: Validate against schema, check required fields, sanitize input

### Ring 2: Security
**Purpose**: Access control, soft delete protection  
**Execution**: Synchronous  
**Use Cases**: Verify permissions, enforce row-level security, audit security events

### Ring 3: Business Logic
**Purpose**: Domain rules, workflows  
**Execution**: Synchronous  
**Use Cases**: Apply business rules, execute workflows, validate business constraints

### Ring 4: Enrichment
**Purpose**: Computed fields, defaults  
**Execution**: Synchronous  
**Use Cases**: Calculate derived fields, apply defaults, generate timestamps

### Ring 5: Database
**Purpose**: SQL execution (handled by repository layer)  
**Execution**: Synchronous  
**Use Cases**: Execute CRUD operations, manage transactions, handle database logic

### Ring 6: Post-Database
**Purpose**: Immediate processing after database operations  
**Execution**: Synchronous  
**Use Cases**: Update caches, validate results, trigger immediate side effects

### Ring 7: Audit
**Purpose**: Change tracking, compliance logging  
**Execution**: Asynchronous  
**Use Cases**: Log changes for audit trails, track user actions, compliance reporting

### Ring 8: Integration
**Purpose**: External APIs, webhooks  
**Execution**: Asynchronous  
**Use Cases**: Call external APIs, send webhooks, sync with external systems

### Ring 9: Notification
**Purpose**: User notifications, real-time updates  
**Execution**: Asynchronous  
**Use Cases**: Send notifications, broadcast real-time updates, trigger UI events

## Supported Operations

- **CREATE**: Insert new records
- **UPDATE**: Modify existing records  
- **DELETE**: Remove records
- **REVERT**: Restore previous record state
- **SELECT**: Read/query records

## Example Observer Implementation

Here's a simple example of a validation observer for Ring 1:

```rust
use async_trait::async_trait;
use crate::observer::{
    traits::{Observer, InputValidationObserver, Operation, ObserverRing},
    context::ObserverContext,
    error::ObserverError,
};
use std::time::Duration;

pub struct RequiredFieldsValidator {
    required_fields: Vec<String>,
}

impl RequiredFieldsValidator {
    pub fn new(required_fields: Vec<String>) -> Self {
        Self { required_fields }
    }
}

impl Observer for RequiredFieldsValidator {
    fn name(&self) -> &'static str {
        "RequiredFieldsValidator"
    }
    
    fn ring(&self) -> ObserverRing {
        ObserverRing::InputValidation
    }
    
    fn applies_to_operation(&self, op: Operation) -> bool {
        matches!(op, Operation::Create | Operation::Update)
    }
    
    fn applies_to_schema(&self, schema: &str) -> bool {
        // Apply to all schemas, or customize per schema
        true
    }
    
    fn timeout(&self) -> Duration {
        Duration::from_secs(2)
    }
    
    fn priority(&self) -> u8 {
        10 // Execute early in validation ring
    }
}

#[async_trait]
impl InputValidationObserver for RequiredFieldsValidator {
    async fn execute(&self, ctx: &mut ObserverContext) -> Result<(), ObserverError> {
        // Check that all required fields are present
        for field in &self.required_fields {
            if !ctx.record_data.contains_key(field) {
                return Err(ObserverError::ValidationError(
                    format!("Required field '{}' is missing", field)
                ));
            }
            
            // Check if field value is not null/empty
            if let Some(value) = ctx.record_data.get(field) {
                if value.is_null() {
                    return Err(ObserverError::ValidationError(
                        format!("Required field '{}' cannot be null", field)
                    ));
                }
            }
        }
        
        Ok(())
    }
}
```

## Registration Example

To register your observer with the pipeline:

```rust
use crate::observer::{
    pipeline::ObserverPipeline,
    traits::ObserverBox,
};

fn register_observers(pipeline: &mut ObserverPipeline) {
    // Register the validator
    let validator = RequiredFieldsValidator::new(vec![
        "name".to_string(),
        "email".to_string(),
    ]);
    
    pipeline.register_observer(
        ObserverBox::InputValidation(Box::new(validator))
    );
}
```

## Key Benefits

1. **Separation of Concerns**: Each ring handles a specific aspect of processing
2. **Extensibility**: Easy to add new observers without modifying existing code
3. **Flexibility**: Observers can be conditionally applied based on operation/schema
4. **Performance**: Async rings don't block the main operation flow
5. **Reliability**: Built-in timeout handling and error management
6. **Testability**: Each observer can be tested independently

## Implementation Guidelines

1. **Keep observers focused**: Each observer should handle one specific concern
2. **Use appropriate rings**: Place observers in the correct ring based on their purpose
3. **Handle errors gracefully**: Return appropriate `ObserverError` types
4. **Set reasonable timeouts**: Don't let observers block operations indefinitely
5. **Consider schema applicability**: Only apply observers where they're needed
6. **Use priorities**: Order observers within rings using the priority system

For implementation details, see the individual ring directories in `implementations/`.