// High-performance observer pipeline with compile-time registration
// Based on superior Rust design from OBSERVER_SYSTEM.md

use std::collections::HashMap;
use std::time::{Duration, Instant};
use tokio::time::timeout;
use serde_json::Value;

use crate::observer::traits::{ObserverRing, Operation, ObserverBox};
use crate::observer::context::ObserverContext;
use crate::observer::error::{ObserverError, ObserverResult};
use crate::observer::stateful_record::StatefulRecord;
use crate::filter::FilterData;

/// Result type for pipeline-level operations that preserves StatefulRecord context
/// Used for internal pipeline-to-pipeline operations
#[derive(Debug)]
pub struct PipelineStatefulResult {
    pub success: bool,
    pub records: Vec<StatefulRecord>,
    pub errors: Vec<ObserverError>,
    pub warnings: Vec<String>,
    pub execution_time: Duration,
    pub rings_executed: Vec<ObserverRing>,
}

impl PipelineStatefulResult {
    /// Convert from ObserverResult, reconstructing StatefulRecords from JSON
    pub fn from_observer_result(result: ObserverResult) -> Self {
        // TODO: In practice, we need to modify ObserverContext to preserve StatefulRecords
        // For now, create empty StatefulRecords as placeholder
        let records = if let Some(json_results) = result.result {
            json_results.into_iter()
                .filter_map(|value| {
                    if let Value::Object(map) = value {
                        // Try to reconstruct StatefulRecord from JSON
                        Some(StatefulRecord::from_select_result(map))
                    } else {
                        None
                    }
                })
                .collect()
        } else {
            Vec::new()
        };

        Self {
            success: result.success,
            records,
            errors: result.errors,
            warnings: result.warnings,
            execution_time: result.execution_time,
            rings_executed: result.rings_executed,
        }
    }
}

/// High-performance observer pipeline with compile-time registration
/// Executes observers in ring order with selective execution and async optimization
pub struct ObserverPipeline {
    // Observer registry by ring
    observers: HashMap<ObserverRing, Vec<ObserverBox>>,
    
    // Configuration
    max_recursion_depth: usize,
}

impl ObserverPipeline {
    /// Create new observer pipeline with empty observer registry
    /// Observers will be registered via register_observer()
    pub fn new() -> Self {
        Self {
            observers: HashMap::new(),
            max_recursion_depth: 3,
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
    
    /// Execute observer pipeline for CRUD operations (CREATE, UPDATE, DELETE, REVERT)
    /// Returns JSON results for Repository layer conversion
    pub async fn execute_crud(
        &self,
        operation: Operation,
        schema_name: String,
        records: Vec<StatefulRecord>,
    ) -> Result<ObserverResult, ObserverError> {
        let start_time = Instant::now();
        
        let mut ctx = ObserverContext::new(operation, schema_name, records);
        
        // Get relevant rings for this operation (performance optimization)
        let relevant_rings = ObserverRing::for_operation(&operation);
        
        tracing::info!(
            "Observer pipeline starting: operation={:?}, schema={}, rings={:?}",
            ctx.operation, ctx.schema_name, relevant_rings
        );
        
        // Execute synchronous rings (0-6) in sequence
        for &ring in relevant_rings.iter().filter(|&&r| r.is_synchronous()) {
            ctx.current_ring = Some(ring);
            
            let should_continue = self.execute_ring(ring, &mut ctx).await?;
            if !should_continue {
                tracing::warn!("Observer pipeline stopped at ring {:?} due to errors", ring);
                break;
            }
        }
        
        // Execute asynchronous rings (7-9) in parallel after database operations
        if ctx.result.is_some() {
            self.execute_async_rings(&relevant_rings, &ctx).await;
        }
        
        let total_time = start_time.elapsed();
        
        // Extract results from StatefulRecords
        let result_data: Vec<Value> = if ctx.result.is_some() {
            ctx.result.unwrap()
        } else {
            // If no result from Ring 5, extract from StatefulRecords
            ctx.records.into_iter()
                .map(|record| Value::Object(record.modified))
                .collect()
        };
        
        Ok(ObserverResult {
            success: ctx.errors.is_empty(),
            result: Some(result_data),
            errors: ctx.errors,
            warnings: ctx.warnings,
            execution_time: total_time,
            rings_executed: relevant_rings,
        })
    }
    
    /// Execute observer pipeline for SELECT operations
    pub async fn execute_select(
        &self,
        schema_name: String,
        filter_data: FilterData,
    ) -> Result<ObserverResult, ObserverError> {
        let start_time = Instant::now();
        
        let mut ctx = ObserverContext::new_select(schema_name, filter_data);
        
        // Get relevant rings for SELECT operation
        let relevant_rings = ObserverRing::for_operation(&Operation::Select);
        
        tracing::info!(
            "Observer SELECT pipeline starting: schema={}, rings={:?}",
            ctx.schema_name, relevant_rings
        );
        
        // Phase 1: Query Preparation (Rings 0-4)
        // Observers work with filter_data and query_metadata
        for &ring in relevant_rings.iter().filter(|&&r| r.is_synchronous() && (r as u8) < 5) {
            ctx.current_ring = Some(ring);
            let should_continue = self.execute_ring(ring, &mut ctx).await?;
            if !should_continue {
                tracing::warn!("SELECT query preparation stopped at ring {:?} due to errors", ring);
                return Ok(ObserverResult::failure(ctx.errors, start_time.elapsed()));
            }
        }
        
        // Phase 2: Database Execution (Ring 5)
        // Creates StatefulRecords from query results
        ctx.current_ring = Some(ObserverRing::Database);
        self.execute_ring(ObserverRing::Database, &mut ctx).await?;
        
        // Phase 3: Result Processing (Rings 6+)
        // Now StatefulRecords are available for processing
        for &ring in relevant_rings.iter().filter(|&&r| r.is_synchronous() && (r as u8) >= 6) {
            ctx.current_ring = Some(ring);
            self.execute_ring(ring, &mut ctx).await?;
        }
        
        // Phase 4: Async Processing (Rings 8-9)
        // Execute in background, don't block response
        self.execute_async_rings(&relevant_rings, &ctx).await;
        
        // Convert StatefulRecords back to JSON for API response
        let result_data: Vec<Value> = if let Some(result) = ctx.result {
            result
        } else {
            ctx.records.into_iter()
                .map(|record| Value::Object(record.modified))
                .collect()
        };
        
        Ok(ObserverResult::success(result_data, start_time.elapsed(), relevant_rings))
    }

    // ========================================
    // Pipeline-level bulk methods (StatefulRecord in/out)
    // ========================================

    /// Create multiple records - StatefulRecord in, StatefulRecord out
    /// For internal pipeline-to-pipeline operations
    pub async fn create_all(&self, schema_name: String, records: Vec<StatefulRecord>) -> Result<PipelineStatefulResult, ObserverError> {
        let result = self.execute_crud(Operation::Create, schema_name, records).await?;
        Ok(PipelineStatefulResult::from_observer_result(result))
    }

    /// Update multiple records - StatefulRecord in, StatefulRecord out
    /// For internal pipeline-to-pipeline operations
    pub async fn update_all(&self, schema_name: String, records: Vec<StatefulRecord>) -> Result<PipelineStatefulResult, ObserverError> {
        let result = self.execute_crud(Operation::Update, schema_name, records).await?;
        Ok(PipelineStatefulResult::from_observer_result(result))
    }

    /// Delete multiple records - StatefulRecord in, StatefulRecord out
    /// For internal pipeline-to-pipeline operations
    pub async fn delete_all(&self, schema_name: String, records: Vec<StatefulRecord>) -> Result<PipelineStatefulResult, ObserverError> {
        let result = self.execute_crud(Operation::Delete, schema_name, records).await?;
        Ok(PipelineStatefulResult::from_observer_result(result))
    }

    /// Revert multiple records - StatefulRecord in, StatefulRecord out
    /// For internal pipeline-to-pipeline operations
    pub async fn revert_all(&self, schema_name: String, records: Vec<StatefulRecord>) -> Result<PipelineStatefulResult, ObserverError> {
        let result = self.execute_crud(Operation::Revert, schema_name, records).await?;
        Ok(PipelineStatefulResult::from_observer_result(result))
    }

    /// Select records with filter - returns StatefulRecord out
    /// For internal pipeline-to-pipeline operations
    pub async fn select_any(&self, schema_name: String, filter_data: FilterData) -> Result<PipelineStatefulResult, ObserverError> {
        let result = self.execute_select(schema_name, filter_data).await?;
        Ok(PipelineStatefulResult::from_observer_result(result))
    }

    /// Execute observers in a specific ring
    async fn execute_ring(&self, ring: ObserverRing, ctx: &mut ObserverContext) -> Result<bool, ObserverError> {
        let observers = match self.observers.get(&ring) {
            Some(obs) => obs,
            None => {
                tracing::debug!("No observers registered for ring {:?}", ring);
                return Ok(true);
            }
        };
        
        tracing::debug!("Executing ring {:?} with {} observers", ring, observers.len());
        
        for observer in observers {
            // Check if observer applies to this operation and schema
            if !observer.applies_to_operation(ctx.operation) {
                tracing::trace!("Observer {} skipped - doesn't apply to operation {:?}", 
                              observer.name(), ctx.operation);
                continue;
            }
            
            if !observer.applies_to_schema(&ctx.schema_name) {
                tracing::trace!("Observer {} skipped - doesn't apply to schema {}", 
                              observer.name(), ctx.schema_name);
                continue;
            }
            
            let observer_start = Instant::now();
            
            // Execute with timeout protection
            let result = timeout(
                observer.timeout(),
                observer.execute_sync(ctx)
            ).await;
            
            let execution_time = observer_start.elapsed();
            
            match result {
                Ok(Ok(_)) => {
                    tracing::debug!(
                        "Observer: {} completed successfully in {:?}",
                        observer.name(), execution_time
                    );
                }
                Ok(Err(error)) => {
                    tracing::warn!(
                        "Observer: {} failed in {:?}: {}",
                        observer.name(), execution_time, error
                    );
                    
                    // Collect error for user feedback
                    ctx.errors.push(error);
                }
                Err(_timeout) => {
                    let timeout_error = ObserverError::TimeoutError(
                        format!("Observer {} timed out after {:?}", 
                                observer.name(), observer.timeout())
                    );
                    
                    tracing::error!(
                        "Observer: {} timed out after {:?}",
                        observer.name(), observer.timeout()
                    );
                    
                    ctx.errors.push(timeout_error);
                }
            }
        }
        
        // Stop execution on errors for pre-database rings
        if !ctx.errors.is_empty() && ring.is_synchronous() && (ring as u8) < 5 {
            return Ok(false);
        }
        
        Ok(true)
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
