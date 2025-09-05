use std::collections::HashMap;
use std::any::{Any, TypeId};
use std::time::Instant;
use serde_json::Value;
use crate::observer::traits::{ObserverRing, Operation};
use crate::observer::error::{ObserverError, ObserverWarning};
use crate::database::record::Record;
use crate::filter::FilterData;

/// Type-safe observer context with Record support
/// This is the main data structure that flows through the observer pipeline
#[derive(Debug)]
pub struct ObserverContext {
    // Core request data
    pub operation: Operation,
    pub schema_name: String,
    
    // Records - using modern Record pattern
    pub records: Vec<Record>,
    
    // SELECT-specific: Query filter data (for SELECT operations)
    pub filter_data: Option<FilterData>,
    
    // Results after database operations (populated by Ring 5)
    pub result: Option<Vec<Value>>,
    
    // Type-safe metadata storage for cross-observer communication
    metadata: HashMap<TypeId, Box<dyn Any + Send>>,
    
    // Performance tracking
    pub start_time: Instant,
    pub current_ring: Option<ObserverRing>,
    
    // Error and warning accumulation
    pub errors: Vec<ObserverError>,
    pub warnings: Vec<ObserverWarning>,
}

impl ObserverContext {
    /// Create new context for CRUD operations (CREATE, UPDATE, DELETE, REVERT)
    pub fn new(
        operation: Operation,
        schema_name: String, 
        records: Vec<Record>
    ) -> Self {
        Self {
            operation,
            schema_name,
            records,
            filter_data: None,
            result: None,
            metadata: HashMap::new(),
            start_time: Instant::now(),
            current_ring: None,
            errors: Vec::new(),
            warnings: Vec::new(),
        }
    }
    
    /// Create new context for SELECT operations
    pub fn new_select(
        schema_name: String,
        filter_data: FilterData,
    ) -> Self {
        Self {
            operation: Operation::Select,
            schema_name,
            records: Vec::new(), // Empty until Ring 5 populates from database
            filter_data: Some(filter_data),
            result: None,
            metadata: HashMap::new(),
            start_time: Instant::now(),
            current_ring: None,
            errors: Vec::new(),
            warnings: Vec::new(),
        }
    }
    
    /// Store typed metadata - compile-time type safety
    pub fn set_metadata<T: Send + 'static>(&mut self, data: T) {
        self.metadata.insert(TypeId::of::<T>(), Box::new(data));
    }
    
    /// Retrieve typed metadata - compile-time type safety
    pub fn get_metadata<T: Send + 'static>(&self) -> Option<&T> {
        self.metadata.get(&TypeId::of::<T>())
            .and_then(|boxed| boxed.downcast_ref::<T>())
    }
    
    /// Retrieve mutable typed metadata
    pub fn get_metadata_mut<T: Send + 'static>(&mut self) -> Option<&mut T> {
        self.metadata.get_mut(&TypeId::of::<T>())
            .and_then(|boxed| boxed.downcast_mut::<T>())
    }
    
    /// Check if metadata of type T exists
    pub fn has_metadata<T: Send + 'static>(&self) -> bool {
        self.metadata.contains_key(&TypeId::of::<T>())
    }
    
    /// Add error to context
    pub fn add_error(&mut self, error: ObserverError) {
        self.errors.push(error);
    }
    
    /// Add warning to context
    pub fn add_warning(&mut self, warning: ObserverWarning) {
        self.warnings.push(warning);
    }
    
    /// Check if context has any errors
    pub fn has_errors(&self) -> bool {
        !self.errors.is_empty()
    }
    
    /// Get total execution time
    pub fn execution_time(&self) -> std::time::Duration {
        self.start_time.elapsed()
    }
    
    // === Record Helper Methods ===
    
    /// Get records that have specific field changes
    pub fn records_with_field_changes(&self, field: &str) -> Vec<&Record> {
        self.records.iter()
            .filter(|record| record.changed(field))
            .collect()
    }
    
    /// Get records by operation type
    pub fn records_by_operation(&self, operation: crate::database::record::Operation) -> Vec<&Record> {
        self.records.iter()
            .filter(|record| record.operation() == operation)
            .collect()
    }
    
    /// Get mutable records by operation type
    pub fn records_by_operation_mut(&mut self, operation: crate::database::record::Operation) -> Vec<&mut Record> {
        self.records.iter_mut()
            .filter(|record| record.operation() == operation)
            .collect()
    }
    
    /// Count records by operation type
    pub fn count_by_operation(&self, operation: crate::database::record::Operation) -> usize {
        self.records.iter()
            .filter(|record| record.operation() == operation)
            .count()
    }
    
    /// Check if any records have changes
    pub fn has_record_changes(&self) -> bool {
        self.records.iter()
            .any(|record| record.has_changes())
    }
}

// Make ObserverContext cloneable for async rings (they get read-only copy)
impl Clone for ObserverContext {
    fn clone(&self) -> Self {
        Self {
            operation: self.operation,
            schema_name: self.schema_name.clone(),
            records: self.records.clone(),
            filter_data: self.filter_data.clone(),
            result: self.result.clone(),
            metadata: HashMap::new(), // Metadata is not cloneable - async observers get fresh context
            start_time: self.start_time,
            current_ring: self.current_ring,
            errors: self.errors.clone(),
            warnings: self.warnings.clone(),
        }
    }
}

/// Strongly-typed metadata structs for cross-observer communication
/// These provide compile-time type safety for observer data sharing

#[derive(Debug, Clone)]
pub struct PreloadedRecords {
    pub records: Vec<Value>,
    pub records_by_id: HashMap<uuid::Uuid, Value>,
    pub requested_count: usize,
    pub found_count: usize,
}

#[derive(Debug, Clone)]
pub struct ValidationResults {
    pub schema_validation_passed: bool,
    pub required_fields_checked: bool,
    pub validated_record_count: usize,
    pub field_errors: HashMap<String, String>,
}

#[derive(Debug, Clone)]
pub struct SecurityCheckResults {
    pub soft_delete_protection_passed: bool,
    pub existence_validation_passed: bool,
    pub access_control_checked: bool,
    pub protected_record_count: usize,
}

#[derive(Debug, Clone)]
pub struct QueryMetadata {
    /// Original filter from API request
    pub original_filter: Option<FilterData>,
    
    /// Enhanced filter with observer modifications
    pub enhanced_filter: Option<FilterData>,
    
    /// Access control filters added by security observers
    pub access_filters: Vec<AccessFilter>,
    
    /// Query optimizations applied
    pub optimizations: Vec<QueryOptimization>,
    
    /// Performance hints
    pub performance_hints: Vec<PerformanceHint>,
    
    /// Fields to select (None = all fields)
    pub select_fields: Option<Vec<String>>,
    
    /// Query execution statistics
    pub execution_stats: Option<QueryExecutionStats>,
}

impl Default for QueryMetadata {
    fn default() -> Self {
        Self {
            original_filter: None,
            enhanced_filter: None,
            access_filters: Vec::new(),
            optimizations: Vec::new(),
            performance_hints: Vec::new(),
            select_fields: None,
            execution_stats: None,
        }
    }
}

#[derive(Debug, Clone)]
pub struct AccessFilter {
    pub observer: String,
    pub filter_type: String,
    pub filter_data: FilterData,
    pub reason: String,
}

#[derive(Debug, Clone)]
pub struct QueryOptimization {
    pub observer: String,
    pub optimization_type: String,
    pub description: String,
    pub original_filter: FilterData,
    pub optimized_filter: FilterData,
}

#[derive(Debug, Clone)]
pub struct PerformanceHint {
    pub observer: String,
    pub hint_type: String,
    pub description: String,
    pub impact: PerformanceImpact,
}

#[derive(Debug, Clone)]
pub enum PerformanceImpact {
    Positive(String), // "Added index hint, 50% faster"
    Negative(String), // "Complex filter, may be slow"
    Neutral(String),  // "No performance impact"
}

#[derive(Debug, Clone)]
pub struct QueryExecutionStats {
    pub query_time_ms: u64,
    pub rows_examined: u64,
    pub rows_returned: u64,
    pub index_usage: Vec<String>,
    pub execution_plan: Option<String>,
}