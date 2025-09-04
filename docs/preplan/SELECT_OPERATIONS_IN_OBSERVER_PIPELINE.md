# SELECT Operations in Observer Pipeline

This document extends the Stateful Record Pattern to include SELECT operations in the observer pipeline, enabling powerful query preprocessing, result enrichment, access control, and audit capabilities.

## Concept Overview

SELECT operations flow through the observer pipeline with a **two-phase execution model**:

**Phase 1 (Rings 0-4): Query Preparation**
- No StatefulRecords exist yet
- Observers work with FilterData and query metadata
- Prepare, validate, and enhance the database query
- Apply access control filters, optimize queries, add security constraints

**Phase 2 (Rings 5-9): Result Processing** 
- Ring 5 executes the prepared query and creates StatefulRecords from results
- Rings 6-9 process the loaded StatefulRecords
- Enable result enrichment, audit logging, cache updates, notifications

```rust
// SELECT operation flow
FilterData -> Rings 0-4 (Query Prep) -> Ring 5 (DB + Create Records) -> Rings 6-9 (Result Processing)
     ↓              ↓                           ↓                              ↓
Query params   Enhanced query              StatefulRecord[]            Processed results
```

## Enhanced Observer Context for SELECT Operations

```rust
// src/observer/context.rs - Enhanced for SELECT operations
pub struct ObserverContext {
    // Core request data
    pub system: SystemContext,
    pub operation: Operation,
    pub schema_name: String,
    pub schema: Schema,
    
    // SELECT-specific: Query preparation phase (Rings 0-4)
    pub filter_data: Option<FilterData>,
    pub query_metadata: QueryMetadata,
    
    // All operations: Stateful records (populated by Ring 5)
    pub records: Vec<StatefulRecord>,
    
    // Type-safe metadata storage
    metadata: HashMap<TypeId, Box<dyn Any + Send>>,
    
    // Performance and error tracking
    pub start_time: std::time::Instant,
    pub current_ring: Option<ObserverRing>,
    pub errors: Vec<ObserverError>,
    pub warnings: Vec<ObserverWarning>,
}

/// Metadata about the query being prepared/executed
#[derive(Debug, Clone, Default)]
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
```

## Enhanced StatefulRecord for SELECT Results

```rust
// src/observer/stateful_record.rs - Enhanced for SELECT operations
impl StatefulRecord {
    /// Create record from SELECT result (Ring 5)
    pub fn from_select_result(data: Map<String, Value>) -> Self {
        let id = data.get("id")
            .and_then(|v| v.as_str())
            .and_then(|s| Uuid::parse_str(s).ok());
        
        Self {
            id,
            original: Some(data.clone()), // SELECT results become "original" state
            modified: data,               // No modifications yet
            operation: RecordOperation::NoChange, // SELECT doesn't modify
            metadata: RecordMetadata {
                api_changes: Vec::new(),     // No API changes for SELECT
                observer_changes: HashMap::new(),
                pipeline_start: Utc::now(),
                ..Default::default()
            },
        }
    }
    
    /// Enrich record with additional data (called by post-processing observers)
    pub fn enrich_field(&mut self, field: &str, value: Value, observer_name: &str) {
        self.modified.insert(field.to_string(), value.clone());
        
        // Track enrichment (different from API changes)
        self.metadata.observer_changes.insert(
            field.to_string(),
            format!("{} (enrichment)", observer_name)
        );
        
        tracing::debug!(
            "Observer {} enriched field '{}' with value: {:?}",
            observer_name, field, value
        );
    }
    
    /// Check if record was enriched by observers
    pub fn was_enriched(&self) -> bool {
        !self.metadata.observer_changes.is_empty()
    }
    
    /// Get enriched fields only
    pub fn get_enriched_fields(&self) -> HashMap<String, &Value> {
        let mut enriched = HashMap::new();
        
        for (field, _observer) in &self.metadata.observer_changes {
            if let Some(value) = self.modified.get(field) {
                enriched.insert(field.clone(), value);
            }
        }
        
        enriched
    }
}

/// Additional operation type for SELECT results
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum RecordOperation {
    Create,
    Update,
    Delete,
    Revert,
    NoChange,    // For SELECT results
    Enriched,    // For SELECT results modified by post-processing observers
}
```

## SELECT-Specific Observer Implementations

### 1. Query Access Control (Ring 2 - Security)

```rust
// src/observer/implementations/query_access_control.rs
#[derive(Default)]
pub struct QueryAccessControl;

impl Observer for QueryAccessControl {
    fn name(&self) -> &'static str { "QueryAccessControl" }
    fn ring(&self) -> ObserverRing { ObserverRing::Security }
    fn applies_to_operation(&self, op: Operation) -> bool {
        matches!(op, Operation::Select)
    }
    fn applies_to_schema(&self, _schema: &str) -> bool { true }
}

#[async_trait]
impl SecurityObserver for QueryAccessControl {
    async fn execute(&self, ctx: &mut ObserverContext) -> Result<(), ObserverError> {
        // Only applies to SELECT operations
        let filter_data = ctx.filter_data.as_mut()
            .ok_or_else(|| ObserverError::SystemError("No filter data for SELECT operation".to_string()))?;
        
        // Get user context from system
        let user = ctx.system.get_user();
        let user_context = [user.id]
            .into_iter()
            .chain(user.access_read.iter().copied())
            .collect::<Vec<Uuid>>();
        
        // Build ACL filter - user must have read access to records
        let acl_filter = FilterData {
            where_clause: Some(json!({
                "$or": [
                    { "access_read": { "$any": user_context } },
                    { "access_edit": { "$any": user_context } },
                    { "access_full": { "$any": user_context } }
                ]
            })),
            ..Default::default()
        };
        
        // Merge ACL filter with existing query
        let combined_filter = if let Some(existing_where) = filter_data.where_clause.clone() {
            FilterData {
                where_clause: Some(json!({
                    "$and": [existing_where, acl_filter.where_clause.unwrap()]
                })),
                ..filter_data.clone()
            }
        } else {
            acl_filter
        };
        
        // Update the filter data with access control
        ctx.filter_data = Some(combined_filter);
        
        // Record what we did for audit
        ctx.query_metadata.access_filters.push(AccessFilter {
            observer: "QueryAccessControl".to_string(),
            filter_type: "ACL".to_string(),
            filter_data: acl_filter,
            reason: format!("Applied ACL filter for user {}", user.id),
        });
        
        tracing::info!("Applied ACL filter for user {} on schema {}", user.id, ctx.schema_name);
        
        Ok(())
    }
}
```

### 2. Query Optimization (Ring 1 - Input Validation)

```rust
// src/observer/implementations/query_optimizer.rs
#[derive(Default)]
pub struct QueryOptimizer;

impl Observer for QueryOptimizer {
    fn name(&self) -> &'static str { "QueryOptimizer" }
    fn ring(&self) -> ObserverRing { ObserverRing::InputValidation }
    fn applies_to_operation(&self, op: Operation) -> bool {
        matches!(op, Operation::Select)
    }
    fn applies_to_schema(&self, _schema: &str) -> bool { true }
}

#[async_trait]
impl InputValidationObserver for QueryOptimizer {
    async fn execute(&self, ctx: &mut ObserverContext) -> Result<(), ObserverError> {
        let filter_data = ctx.filter_data.as_mut()
            .ok_or_else(|| ObserverError::SystemError("No filter data for SELECT operation".to_string()))?;
        
        let original_filter = filter_data.clone();
        let mut optimizations = Vec::new();
        
        // Optimization 1: Add index hints for common query patterns
        if let Some(where_clause) = &filter_data.where_clause {
            if where_clause.get("id").is_some() {
                // ID-based queries should use primary key index
                ctx.query_metadata.performance_hints.push(PerformanceHint {
                    observer: "QueryOptimizer".to_string(),
                    hint_type: "index_hint".to_string(),
                    description: "ID-based query will use primary key index".to_string(),
                    impact: PerformanceImpact::Positive("Primary key lookup, very fast".to_string()),
                });
            }
            
            // Check for queries that might be slow
            if where_clause.get("$text").is_some() {
                ctx.query_metadata.performance_hints.push(PerformanceHint {
                    observer: "QueryOptimizer".to_string(),
                    hint_type: "performance_warning".to_string(),
                    description: "Full-text search may be slow on large datasets".to_string(),
                    impact: PerformanceImpact::Negative("Full-text search can be expensive".to_string()),
                });
            }
        }
        
        // Optimization 2: Limit result set size for safety
        if filter_data.limit.is_none() || filter_data.limit.unwrap() > 1000 {
            let old_limit = filter_data.limit;
            filter_data.limit = Some(1000.min(filter_data.limit.unwrap_or(1000)));
            
            optimizations.push(QueryOptimization {
                observer: "QueryOptimizer".to_string(),
                optimization_type: "limit_safety".to_string(),
                description: format!("Limited result set from {:?} to {} for safety", old_limit, filter_data.limit.unwrap()),
                original_filter: original_filter.clone(),
                optimized_filter: filter_data.clone(),
            });
            
            tracing::info!("Applied safety limit: {} records max", filter_data.limit.unwrap());
        }
        
        // Optimization 3: Add default ordering for consistent results
        if filter_data.order.is_none() {
            filter_data.order = Some("created_at desc".to_string());
            
            optimizations.push(QueryOptimization {
                observer: "QueryOptimizer".to_string(),
                optimization_type: "default_ordering".to_string(),
                description: "Added default ordering by created_at desc".to_string(),
                original_filter: original_filter.clone(),
                optimized_filter: filter_data.clone(),
            });
        }
        
        // Store optimizations for audit
        ctx.query_metadata.optimizations.extend(optimizations);
        
        Ok(())
    }
}
```

### 3. Result Enricher (Ring 6 - Post-Database)

```rust
// src/observer/implementations/result_enricher.rs
#[derive(Default)]
pub struct ResultEnricher;

impl Observer for ResultEnricher {
    fn name(&self) -> &'static str { "ResultEnricher" }
    fn ring(&self) -> ObserverRing { ObserverRing::PostDatabase }
    fn applies_to_operation(&self, op: Operation) -> bool {
        matches!(op, Operation::Select)
    }
    fn applies_to_schema(&self, _schema: &str) -> bool { true }
}

#[async_trait]
impl PostDatabaseObserver for ResultEnricher {
    async fn execute(&self, ctx: &mut ObserverContext) -> Result<(), ObserverError> {
        // Now we have StatefulRecords from Ring 5 database execution
        for record in &mut ctx.records {
            // Add computed fields based on existing data
            if let Some(created_at) = record.get_field("created_at") {
                if let Some(created_str) = created_at.as_str() {
                    if let Ok(created_time) = DateTime::parse_from_rfc3339(created_str) {
                        let age_days = (Utc::now() - created_time.with_timezone(&Utc)).num_days();
                        
                        record.enrich_field(
                            "_computed_age_days", 
                            Value::Number(age_days.into()), 
                            "ResultEnricher"
                        );
                    }
                }
            }
            
            // Add display names or formatted fields
            if let Some(user_id) = record.get_field("user_id") {
                if let Some(user_id_str) = user_id.as_str() {
                    // Look up user display name (could cache this)
                    let display_name = self.get_user_display_name(&ctx.system, user_id_str).await
                        .unwrap_or_else(|_| "Unknown User".to_string());
                    
                    record.enrich_field(
                        "_computed_user_display_name",
                        Value::String(display_name),
                        "ResultEnricher"
                    );
                }
            }
            
            // Add permissions metadata for frontend
            let user = ctx.system.get_user();
            let can_edit = self.user_can_edit_record(&user, record);
            let can_delete = self.user_can_delete_record(&user, record);
            
            record.enrich_field(
                "_permissions",
                json!({
                    "can_edit": can_edit,
                    "can_delete": can_delete,
                    "can_read": true // They can read it since they got it
                }),
                "ResultEnricher"
            );
        }
        
        tracing::info!("Enriched {} SELECT results with computed fields", ctx.records.len());
        
        Ok(())
    }
}

impl ResultEnricher {
    async fn get_user_display_name(&self, system: &SystemContext, user_id: &str) -> Result<String, ObserverError> {
        // Implementation to look up user display name
        // Could use a cache here for performance
        let user_repo = Repository::new("users", &system.db_context);
        let filter = FilterData {
            where_clause: Some(json!({ "id": user_id })),
            ..Default::default()
        };
        
        if let Some(user) = user_repo.select_one(filter).await?.get(0) {
            Ok(user.get("name")
                .and_then(|v| v.as_str())
                .unwrap_or("Unknown")
                .to_string())
        } else {
            Ok("Unknown User".to_string())
        }
    }
    
    fn user_can_edit_record(&self, user: &User, record: &StatefulRecord) -> bool {
        // Check if user has edit permissions for this record
        if user.access == "root" || user.access == "full" {
            return true;
        }
        
        // Check ACL fields
        if let Some(access_edit) = record.get_field("access_edit") {
            if let Some(edit_array) = access_edit.as_array() {
                return edit_array.iter().any(|v| {
                    v.as_str().and_then(|s| Uuid::parse_str(s).ok()) == Some(user.id)
                });
            }
        }
        
        false
    }
    
    fn user_can_delete_record(&self, user: &User, record: &StatefulRecord) -> bool {
        // Similar logic for delete permissions
        user.access == "root" || user.access == "full"
    }
}
```

### 4. Query Audit Logger (Ring 7 - Audit)

```rust
// src/observer/implementations/query_audit_logger.rs
#[derive(Default)]
pub struct QueryAuditLogger;

impl Observer for QueryAuditLogger {
    fn name(&self) -> &'static str { "QueryAuditLogger" }
    fn ring(&self) -> ObserverRing { ObserverRing::Audit }
    fn applies_to_operation(&self, op: Operation) -> bool {
        matches!(op, Operation::Select)
    }
    fn applies_to_schema(&self, _schema: &str) -> bool { true }
}

#[async_trait]
impl AuditObserver for QueryAuditLogger {
    async fn execute(&self, ctx: &ObserverContext) -> Result<(), ObserverError> {
        let user = ctx.system.get_user();
        let execution_stats = &ctx.query_metadata.execution_stats;
        
        // Log data access for compliance/security
        let audit_entry = json!({
            "event_type": "data_access",
            "timestamp": Utc::now().to_rfc3339(),
            "user_id": user.id,
            "user_auth": user.auth,
            "schema": ctx.schema_name,
            "operation": "select",
            "query_metadata": {
                "original_filter": ctx.query_metadata.original_filter,
                "access_filters_applied": ctx.query_metadata.access_filters.len(),
                "optimizations_applied": ctx.query_metadata.optimizations.len(),
                "performance_hints": ctx.query_metadata.performance_hints.len(),
            },
            "results": {
                "records_returned": ctx.records.len(),
                "records_enriched": ctx.records.iter().filter(|r| r.was_enriched()).count(),
                "execution_time_ms": execution_stats.as_ref().map(|s| s.query_time_ms),
                "rows_examined": execution_stats.as_ref().map(|s| s.rows_examined),
            },
            "tenant_db": ctx.system.tenant_db,
        });
        
        // Log to audit system (could be database, file, external service)
        tracing::info!(target: "audit", "{}", audit_entry);
        
        // For sensitive schemas, log individual record access
        if self.is_sensitive_schema(&ctx.schema_name) {
            for record in &ctx.records {
                let record_audit = json!({
                    "event_type": "sensitive_data_access",
                    "timestamp": Utc::now().to_rfc3339(),
                    "user_id": user.id,
                    "schema": ctx.schema_name,
                    "record_id": record.id,
                    "tenant_db": ctx.system.tenant_db,
                });
                
                tracing::warn!(target: "audit", "{}", record_audit);
            }
        }
        
        Ok(())
    }
}

impl QueryAuditLogger {
    fn is_sensitive_schema(&self, schema: &str) -> bool {
        matches!(schema, "users" | "payments" | "personal_data" | "medical_records")
    }
}
```

### 5. Enhanced Database Observer (Ring 5)

```rust
// src/observer/implementations/sql_select_observer.rs
#[async_trait]
impl DatabaseObserver for SqlSelectObserver {
    async fn execute(&self, ctx: &mut ObserverContext) -> Result<(), ObserverError> {
        let query_start = std::time::Instant::now();
        
        // Get the prepared filter from previous rings
        let filter_data = ctx.filter_data.clone()
            .unwrap_or_default();
        
        // Execute the enhanced query with all optimizations and access controls
        let repo = Repository::new(&ctx.schema_name, &ctx.system.db_context);
        let raw_results = repo.select_any(filter_data).await?;
        
        let query_time = query_start.elapsed();
        
        // Convert raw results to StatefulRecords for post-processing rings
        let stateful_records: Vec<StatefulRecord> = raw_results.into_iter()
            .map(|raw_record| StatefulRecord::from_select_result(raw_record))
            .collect();
        
        // Update context with results
        ctx.records = stateful_records;
        
        // Store execution statistics for audit and performance monitoring
        ctx.query_metadata.execution_stats = Some(QueryExecutionStats {
            query_time_ms: query_time.as_millis() as u64,
            rows_examined: ctx.records.len() as u64, // Simplified
            rows_returned: ctx.records.len() as u64,
            index_usage: vec![], // Would be populated by database profiling
            execution_plan: None, // Could include EXPLAIN output
        });
        
        tracing::info!(
            "SELECT executed: {} records returned in {}ms",
            ctx.records.len(),
            query_time.as_millis()
        );
        
        Ok(())
    }
}
```

## Updated Pipeline Execution for SELECT Operations

```rust
// src/observer/pipeline.rs - Enhanced for SELECT operations
impl ObserverPipeline {
    pub async fn execute_select(
        &self,
        system: SystemContext,
        schema: Schema,
        filter_data: FilterData,
    ) -> Result<ObserverResult, ObserverError> {
        let start_time = std::time::Instant::now();
        
        // Create context for SELECT operation
        let mut ctx = ObserverContext {
            system,
            operation: Operation::Select,
            schema_name: schema.name.clone(),
            schema,
            records: Vec::new(), // Empty until Ring 5
            filter_data: Some(filter_data.clone()),
            query_metadata: QueryMetadata {
                original_filter: Some(filter_data),
                ..Default::default()
            },
            metadata: HashMap::new(),
            start_time,
            current_ring: None,
            errors: Vec::new(),
            warnings: Vec::new(),
        };
        
        // Phase 1: Query Preparation (Rings 0-4)
        // Observers work with filter_data and query_metadata
        for ring in [ObserverRing::DataPreparation, ObserverRing::InputValidation, ObserverRing::Security] {
            ctx.current_ring = Some(ring);
            let should_continue = self.execute_ring(ring, &mut ctx).await?;
            if !should_continue {
                tracing::warn!("SELECT query preparation stopped at ring {:?} due to errors", ring);
                return Ok(ObserverResult {
                    success: false,
                    result: None,
                    errors: ctx.errors,
                    warnings: ctx.warnings,
                    execution_time: start_time.elapsed(),
                    rings_executed: vec![ring],
                });
            }
        }
        
        // Phase 2: Database Execution (Ring 5)
        // Creates StatefulRecords from query results
        ctx.current_ring = Some(ObserverRing::Database);
        self.execute_ring(ObserverRing::Database, &mut ctx).await?;
        
        // Phase 3: Result Processing (Rings 6-9)
        // Now StatefulRecords are available for processing
        for ring in [ObserverRing::PostDatabase, ObserverRing::Audit] {
            ctx.current_ring = Some(ring);
            self.execute_ring(ring, &mut ctx).await?;
        }
        
        // Phase 4: Async Processing (Rings 8-9)
        // Execute in background, don't block response
        self.execute_async_rings(&[ObserverRing::Integration, ObserverRing::Notification], &ctx).await;
        
        // Convert StatefulRecords back to JSON for API response
        let result_data: Vec<serde_json::Value> = ctx.records.into_iter()
            .map(|record| serde_json::Value::Object(record.modified))
            .collect();
        
        Ok(ObserverResult {
            success: ctx.errors.is_empty(),
            result: Some(result_data),
            errors: ctx.errors,
            warnings: ctx.warnings,
            execution_time: start_time.elapsed(),
            rings_executed: vec![
                ObserverRing::DataPreparation,
                ObserverRing::InputValidation, 
                ObserverRing::Security,
                ObserverRing::Database,
                ObserverRing::PostDatabase,
                ObserverRing::Audit,
            ],
        })
    }
}
```

## Usage in Route Handlers

```rust
// src/handlers/protected/data/find.rs
pub async fn find_post(
    Path(schema): Path<String>,
    Extension(ctx): Extension<RequestContext>,
    Json(filter_data): Json<FilterData>,
) -> Result<ApiResponse<Vec<serde_json::Value>>, ApiError> {
    
    // Load schema
    let schema_obj = ctx.system.load_schema(&schema).await?;
    
    // Execute SELECT operation through observer pipeline
    let pipeline = ObserverPipeline::new();
    let observer_result = pipeline.execute_select(
        ctx.system.clone(),
        schema_obj,
        filter_data,
    ).await?;
    
    if !observer_result.success {
        return Err(ApiError::validation_error(
            format!("Query validation failed: {} errors", observer_result.errors.len())
        ));
    }
    
    let results = observer_result.result.unwrap_or_default();
    
    Ok(ApiResponse::success(results))
}
```

## Benefits of SELECT Operations in Observer Pipeline

### 1. **Unified Security Model**
All data access goes through the same security observers - no bypassing ACL

### 2. **Query Enhancement**
- Automatic access control filtering
- Performance optimizations 
- Safety limits and hints

### 3. **Result Enrichment**
- Add computed fields
- Include permissions metadata
- Format data for frontend

### 4. **Complete Audit Trail**
- Log all data access attempts
- Track query performance
- Monitor sensitive data access

### 5. **Consistency**
Same observer pattern for all operations - no special cases

### 6. **Performance Intelligence**
- Query optimization hints
- Automatic performance warnings
- Execution statistics collection

This extension makes the observer system truly universal - every database operation, including SELECT, benefits from the full pipeline of validation, security, optimization, and monitoring.