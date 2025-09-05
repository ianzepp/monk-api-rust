// High-performance observer pipeline with compile-time registration
// Based on superior Rust design from OBSERVER_SYSTEM.md

use std::collections::HashMap;
use std::time::{Duration, Instant};
use tokio::time::timeout;
use serde_json::Value;

use crate::observer::traits::{ObserverRing, Operation, ObserverBox};
use crate::observer::context::ObserverContext;
use crate::observer::error::{ObserverError, ObserverResult};
use crate::filter::FilterData;


/// High-performance observer pipeline with compile-time registration
/// Executes observers in ring order with selective execution and async optimization
pub struct ObserverPipeline {
    // Observer registry by ring
    observers: HashMap<ObserverRing, Vec<ObserverBox>>,
}

impl ObserverPipeline {
    /// Create new observer pipeline with empty observer registry
    /// Observers will be registered via register_observer()
    pub fn new() -> Self {
        Self {
            observers: HashMap::new(),
        }
    }
    
    /// Register an observer (type-safe registration)
    pub fn register_observer(&mut self, observer: ObserverBox) {
        let ring = observer.ring();
        let name = observer.name();
        self.observers
            .entry(ring)
            .or_insert_with(Vec::new)
            .push(observer);
        
        tracing::debug!("Registered observer '{}' for ring {:?}", name, ring);
    }
    
    /// Execute modification operations (CREATE, UPDATE, DELETE, REVERT)
    pub async fn modify(
        &self,
        operation: Operation,
        schema_name: impl Into<String>,
        records: Vec<crate::database::record::Record>,
        pool: sqlx::PgPool,
    ) -> Result<Vec<crate::database::record::Record>, ObserverError> {
        let ctx = ObserverContext::new(operation, schema_name.into(), records, pool);
        let result = self.execute_internal(ctx).await?;
        self.extract_records(result)
    }
    
    /// Execute SELECT operations
    pub async fn select(
        &self,
        schema_name: impl Into<String>,
        filter_data: FilterData,
        pool: sqlx::PgPool,
    ) -> Result<Vec<crate::database::record::Record>, ObserverError> {
        let ctx = ObserverContext::new_select(schema_name.into(), filter_data, pool);
        let result = self.execute_internal(ctx).await?;
        self.extract_records(result)
    }
    
    /// Internal pipeline execution - handles all operation types
    async fn execute_internal(&self, mut ctx: ObserverContext) -> Result<ObserverResult, ObserverError> {
        let start_time = Instant::now();
        let relevant_rings = ObserverRing::for_operation(&ctx.operation);
        
        tracing::info!(
            "Pipeline starting: op={:?}, schema={}, rings={:?}",
            ctx.operation, ctx.schema_name, relevant_rings
        );
        
        // Execute all synchronous rings in order
        for &ring in relevant_rings.iter().filter(|r| r.is_synchronous()) {
            ctx.current_ring = Some(ring);
            
            let should_continue = self.execute_ring(ring, &mut ctx).await?;
            if !should_continue {
                tracing::warn!("Pipeline stopped at ring {:?} due to errors", ring);
                break;
            }
        }
        
        // Execute async rings (when implemented)
        if ctx.result.is_some() {
            self.execute_async_rings(&relevant_rings, &ctx).await;
        }
        
        self.build_result(ctx, start_time.elapsed(), relevant_rings)
    }

    /// Build final result from context
    fn build_result(&self, ctx: ObserverContext, duration: Duration, rings: Vec<ObserverRing>) -> Result<ObserverResult, ObserverError> {
        let result_data = ctx.result.unwrap_or_else(|| {
            ctx.records.into_iter().map(|record| record.to_json()).collect()
        });
        
        Ok(ObserverResult {
            success: ctx.errors.is_empty(),
            result: Some(result_data),
            errors: ctx.errors,
            warnings: ctx.warnings,
            execution_time: duration,
            rings_executed: rings,
        })
    }
    
    /// Extract Records from ObserverResult
    fn extract_records(&self, result: ObserverResult) -> Result<Vec<crate::database::record::Record>, ObserverError> {
        if !result.success {
            return Err(ObserverError::ValidationError(
                format!("Pipeline failed with {} errors", result.errors.len())
            ));
        }
        
        let json_results = result.result.unwrap_or_default();
        let mut records = Vec::new();
        
        for value in json_results {
            if let Value::Object(map) = value {
                let record = crate::database::record::Record::from_sql_data(
                    map.into_iter().collect()
                );
                records.push(record);
            } else {
                return Err(ObserverError::ValidationError(
                    "Invalid result format - expected JSON object".to_string()
                ));
            }
        }
        
        Ok(records)
    }

    /// Execute observers in a specific ring
    async fn execute_ring(&self, ring: ObserverRing, ctx: &mut ObserverContext) -> Result<bool, ObserverError> {
        let Some(observers) = self.observers.get(&ring) else {
            tracing::debug!("No observers registered for ring {:?}", ring);
            return Ok(true);
        };
        
        tracing::debug!("Executing ring {:?} with {} observers", ring, observers.len());
        
        for observer in observers {
            if !observer.applies_to_operation(ctx.operation) || !observer.applies_to_schema(&ctx.schema_name) {
                continue;
            }
            
            self.execute_observer(observer, ctx).await;
        }
        
        // Stop on errors for pre-database rings
        Ok(ctx.errors.is_empty() || (ring as u8) >= 5)
    }
    
    /// Execute a single observer with timeout and error handling
    async fn execute_observer(&self, observer: &ObserverBox, ctx: &mut ObserverContext) {
        let start = Instant::now();
        let result = timeout(observer.timeout(), observer.execute_sync(ctx)).await;
        let duration = start.elapsed();
        
        match result {
            Ok(Ok(_)) => {
                tracing::debug!("Observer {} completed in {:?}", observer.name(), duration);
            }
            Ok(Err(error)) => {
                tracing::warn!("Observer {} failed in {:?}: {}", observer.name(), duration, error);
                ctx.errors.push(error);
            }
            Err(_) => {
                let timeout_error = ObserverError::TimeoutError(
                    format!("Observer {} timed out after {:?}", observer.name(), observer.timeout())
                );
                tracing::error!("Observer {} timed out after {:?}", observer.name(), observer.timeout());
                ctx.errors.push(timeout_error);
            }
        }
    }
    
    /// Execute asynchronous rings in parallel (non-blocking)
    /// TODO: Implement full async execution when needed
    async fn execute_async_rings(&self, _relevant_rings: &[ObserverRing], _ctx: &ObserverContext) {
        tracing::debug!("Async ring execution not yet implemented - skipping");
        // For now, skip async execution to get the framework compiling
        // This can be implemented later when specific async observers are needed
    }
}

impl Default for ObserverPipeline {
    fn default() -> Self {
        Self::new()
    }
}
