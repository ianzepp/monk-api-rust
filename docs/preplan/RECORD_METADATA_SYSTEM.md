# Record Metadata System

This document outlines a generalized **Record Metadata System** that separates actual record data from system-level information, computed fields, permissions, and other metadata. This provides a clean API for clients to request specific metadata without polluting the core data.

## Problem with Current Approach

The current enrichment pattern adds metadata directly to record data:

```rust
// ❌ Current approach - pollutes record data
{
  "id": "123",
  "name": "John Smith",
  "email": "john@example.com",          // ← User data
  "_computed_age_days": 45,             // ← Computed metadata mixed in
  "_permissions": {"can_edit": true},   // ← Permission metadata mixed in
  "created_at": "2024-01-01T00:00:00Z", // ← System metadata mixed in
  "access_read": ["uuid1", "uuid2"]     // ← System metadata mixed in
}
```

**Problems:**
1. **Data pollution**: System metadata mixed with user data
2. **Namespace conflicts**: `_computed_*` fields could conflict with user fields
3. **No granular control**: Can't selectively include/exclude metadata types
4. **Type confusion**: Unclear what's user data vs system-generated

## Superior Metadata System

```rust
// ✅ Clean separation of data and metadata
{
  "data": {
    "id": "123",
    "name": "John Smith", 
    "email": "john@example.com"     // ← Only user data
  },
  "metadata": {
    "system": {
      "created_at": "2024-01-01T00:00:00Z",
      "updated_at": "2024-01-15T10:30:00Z",
      "trashed_at": null,
      "access_read": ["uuid1", "uuid2"],
      "access_edit": ["uuid1"],
      "version": 5
    },
    "computed": {
      "age_days": 45,
      "display_name": "John Smith",
      "status": "active"
    },
    "permissions": {
      "can_read": true,
      "can_edit": true,
      "can_delete": false,
      "can_share": true
    },
    "relationships": {
      "user_count": 150,
      "related_records": ["uuid3", "uuid4"],
      "parent_schema": "organizations"
    },
    "processing": {
      "enriched_by": ["ResultEnricher", "PermissionCalculator"],
      "processing_time_ms": 12,
      "cache_hit": true
    }
  }
}
```

## Core Architecture

### 1. Enhanced StatefulRecord with Metadata

```rust
// src/observer/stateful_record.rs - Enhanced with metadata system
#[derive(Debug, Clone)]
pub struct StatefulRecord {
    /// Unique identifier for this record
    pub id: Option<Uuid>,
    
    /// Original state from database (None for CREATE operations)
    pub original: Option<Map<String, Value>>,
    
    /// Current modified state (user data only)
    pub modified: Map<String, Value>,
    
    /// Operation type for this record
    pub operation: RecordOperation,
    
    /// Change tracking metadata (internal)
    pub metadata: RecordMetadata,
    
    /// NEW: Structured metadata for API responses
    pub response_metadata: RecordResponseMetadata,
}

/// Structured metadata that can be included in API responses
#[derive(Debug, Clone, Default)]
pub struct RecordResponseMetadata {
    /// System-level fields (created_at, updated_at, ACL, etc.)
    pub system: SystemMetadata,
    
    /// Computed fields added by observers
    pub computed: HashMap<String, Value>,
    
    /// Permission information for this record
    pub permissions: PermissionMetadata,
    
    /// Relationship information
    pub relationships: RelationshipMetadata,
    
    /// Processing information (debugging, performance)
    pub processing: ProcessingMetadata,
}

#[derive(Debug, Clone, Default)]
pub struct SystemMetadata {
    pub created_at: Option<DateTime<Utc>>,
    pub updated_at: Option<DateTime<Utc>>,
    pub trashed_at: Option<DateTime<Utc>>,
    pub deleted_at: Option<DateTime<Utc>>,
    pub access_read: Vec<Uuid>,
    pub access_edit: Vec<Uuid>,
    pub access_full: Vec<Uuid>,
    pub access_deny: Vec<Uuid>,
    pub version: Option<i64>,
    pub tenant_id: Option<Uuid>,
}

#[derive(Debug, Clone, Default)]
pub struct PermissionMetadata {
    pub can_read: bool,
    pub can_edit: bool,
    pub can_delete: bool,
    pub can_share: bool,
    pub effective_access_level: AccessLevel,
    pub permission_source: String, // "owner", "acl", "role", etc.
}

#[derive(Debug, Clone)]
pub enum AccessLevel {
    None,
    Read,
    Edit,
    Full,
    Root,
}

#[derive(Debug, Clone, Default)]
pub struct RelationshipMetadata {
    /// Count of related records in other schemas
    pub related_counts: HashMap<String, i64>, // schema_name -> count
    
    /// Direct relationships
    pub relationships: HashMap<String, Vec<Uuid>>,
    
    /// Parent/child hierarchy info
    pub parent_schema: Option<String>,
    pub child_schemas: Vec<String>,
}

#[derive(Debug, Clone, Default)]
pub struct ProcessingMetadata {
    /// Observers that enriched this record
    pub enriched_by: Vec<String>,
    
    /// Time spent processing this record
    pub processing_time_ms: Option<u64>,
    
    /// Whether data came from cache
    pub cache_hit: bool,
    
    /// Database query performance
    pub query_stats: Option<RecordQueryStats>,
}

#[derive(Debug, Clone)]
pub struct RecordQueryStats {
    pub execution_time_ms: u64,
    pub rows_examined: u64,
    pub index_used: Option<String>,
}
```

### 2. Enhanced StatefulRecord Methods

```rust
impl StatefulRecord {
    /// Extract system metadata from record data
    pub fn extract_system_metadata(&mut self) {
        let system_fields = [
            "created_at", "updated_at", "trashed_at", "deleted_at",
            "access_read", "access_edit", "access_full", "access_deny"
        ];
        
        for field in system_fields {
            if let Some(value) = self.modified.remove(field) {
                match field {
                    "created_at" => {
                        if let Some(dt_str) = value.as_str() {
                            self.response_metadata.system.created_at = 
                                DateTime::parse_from_rfc3339(dt_str).ok()
                                .map(|dt| dt.with_timezone(&Utc));
                        }
                    }
                    "updated_at" => {
                        if let Some(dt_str) = value.as_str() {
                            self.response_metadata.system.updated_at = 
                                DateTime::parse_from_rfc3339(dt_str).ok()
                                .map(|dt| dt.with_timezone(&Utc));
                        }
                    }
                    "access_read" => {
                        if let Some(array) = value.as_array() {
                            self.response_metadata.system.access_read = array.iter()
                                .filter_map(|v| v.as_str().and_then(|s| Uuid::parse_str(s).ok()))
                                .collect();
                        }
                    }
                    "access_edit" => {
                        if let Some(array) = value.as_array() {
                            self.response_metadata.system.access_edit = array.iter()
                                .filter_map(|v| v.as_str().and_then(|s| Uuid::parse_str(s).ok()))
                                .collect();
                        }
                    }
                    // ... other system fields
                    _ => {}
                }
            }
        }
    }
    
    /// Add computed metadata (replaces enrichment)
    pub fn add_computed_field(&mut self, field: &str, value: Value, observer_name: &str) {
        self.response_metadata.computed.insert(field.to_string(), value);
        self.response_metadata.processing.enriched_by.push(observer_name.to_string());
        
        tracing::debug!(
            "Observer {} added computed field '{}' to metadata",
            observer_name, field
        );
    }
    
    /// Set permission metadata
    pub fn set_permissions(&mut self, permissions: PermissionMetadata) {
        self.response_metadata.permissions = permissions;
    }
    
    /// Add relationship metadata
    pub fn add_relationship_count(&mut self, schema: &str, count: i64) {
        self.response_metadata.relationships.related_counts.insert(schema.to_string(), count);
    }
    
    /// Build API response with selective metadata inclusion
    pub fn to_api_response(&self, metadata_options: &MetadataOptions) -> serde_json::Value {
        let mut response = json!({
            "data": self.modified
        });
        
        if metadata_options.should_include_any() {
            let mut metadata = json!({});
            
            if metadata_options.include_system {
                metadata["system"] = json!({
                    "created_at": self.response_metadata.system.created_at.map(|dt| dt.to_rfc3339()),
                    "updated_at": self.response_metadata.system.updated_at.map(|dt| dt.to_rfc3339()),
                    "access_read": self.response_metadata.system.access_read,
                    "access_edit": self.response_metadata.system.access_edit,
                    "version": self.response_metadata.system.version,
                });
            }
            
            if metadata_options.include_computed {
                metadata["computed"] = json!(self.response_metadata.computed);
            }
            
            if metadata_options.include_permissions {
                metadata["permissions"] = json!({
                    "can_read": self.response_metadata.permissions.can_read,
                    "can_edit": self.response_metadata.permissions.can_edit,
                    "can_delete": self.response_metadata.permissions.can_delete,
                    "effective_access_level": format!("{:?}", self.response_metadata.permissions.effective_access_level),
                });
            }
            
            if metadata_options.include_relationships {
                metadata["relationships"] = json!(self.response_metadata.relationships);
            }
            
            if metadata_options.include_processing {
                metadata["processing"] = json!(self.response_metadata.processing);
            }
            
            // Apply field-specific filters
            if let Some(specific_fields) = &metadata_options.specific_fields {
                metadata = self.filter_metadata_fields(metadata, specific_fields);
            }
            
            response["metadata"] = metadata;
        }
        
        response
    }
    
    fn filter_metadata_fields(&self, metadata: Value, fields: &[String]) -> Value {
        // Implementation to filter metadata to only include specified fields
        let mut filtered = json!({});
        
        for field in fields {
            // Support dot notation: "system.created_at", "permissions.can_edit"
            let parts: Vec<&str> = field.split('.').collect();
            if parts.len() == 2 {
                let category = parts[0];
                let field_name = parts[1];
                
                if let Some(category_data) = metadata.get(category) {
                    if let Some(field_value) = category_data.get(field_name) {
                        if !filtered.get(category).is_some() {
                            filtered[category] = json!({});
                        }
                        filtered[category][field_name] = field_value.clone();
                    }
                }
            }
        }
        
        filtered
    }
}
```

### 3. Metadata Options from Query Parameters

```rust
// src/api/metadata_options.rs
#[derive(Debug, Clone, Default)]
pub struct MetadataOptions {
    pub include_system: bool,
    pub include_computed: bool,
    pub include_permissions: bool,
    pub include_relationships: bool,
    pub include_processing: bool,
    
    /// Specific fields to include (dot notation: "system.created_at")
    pub specific_fields: Option<Vec<String>>,
}

impl MetadataOptions {
    /// Parse from query parameter
    /// Examples:
    /// ?metadata=true -> include all metadata
    /// ?metadata=system,permissions -> include only system and permissions
    /// ?metadata=system.created_at,permissions.can_edit -> specific fields only
    pub fn from_query_param(metadata_param: Option<&str>) -> Self {
        match metadata_param {
            None => Self::default(),
            Some("true") => Self::all(),
            Some("false") | Some("") => Self::none(),
            Some(param_value) => Self::parse_specific(param_value),
        }
    }
    
    pub fn all() -> Self {
        Self {
            include_system: true,
            include_computed: true,
            include_permissions: true,
            include_relationships: true,
            include_processing: false, // Usually too verbose for normal use
            specific_fields: None,
        }
    }
    
    pub fn none() -> Self {
        Self::default()
    }
    
    pub fn parse_specific(param_value: &str) -> Self {
        let fields: Vec<String> = param_value.split(',')
            .map(|s| s.trim().to_string())
            .collect();
        
        let mut options = Self::default();
        let mut specific_fields = Vec::new();
        
        for field in fields {
            match field.as_str() {
                "system" => options.include_system = true,
                "computed" => options.include_computed = true,
                "permissions" => options.include_permissions = true,
                "relationships" => options.include_relationships = true,
                "processing" => options.include_processing = true,
                field_name if field_name.contains('.') => {
                    // Specific field like "system.created_at"
                    specific_fields.push(field_name.to_string());
                    
                    // Auto-enable the category
                    let category = field_name.split('.').next().unwrap_or("");
                    match category {
                        "system" => options.include_system = true,
                        "computed" => options.include_computed = true,
                        "permissions" => options.include_permissions = true,
                        "relationships" => options.include_relationships = true,
                        "processing" => options.include_processing = true,
                        _ => {}
                    }
                }
                _ => {
                    // Unknown field - could warn or ignore
                    tracing::warn!("Unknown metadata field requested: {}", field);
                }
            }
        }
        
        if !specific_fields.is_empty() {
            options.specific_fields = Some(specific_fields);
        }
        
        options
    }
    
    pub fn should_include_any(&self) -> bool {
        self.include_system || self.include_computed || self.include_permissions || 
        self.include_relationships || self.include_processing
    }
}
```

### 4. Enhanced Observer Implementations

```rust
// src/observer/implementations/metadata_enricher.rs
#[derive(Default)]
pub struct MetadataEnricher;

impl Observer for MetadataEnricher {
    fn name(&self) -> &'static str { "MetadataEnricher" }
    fn ring(&self) -> ObserverRing { ObserverRing::PostDatabase }
    fn applies_to_operation(&self, op: Operation) -> bool {
        matches!(op, Operation::Select)
    }
    fn applies_to_schema(&self, _schema: &str) -> bool { true }
}

#[async_trait]
impl PostDatabaseObserver for MetadataEnricher {
    async fn execute(&self, ctx: &mut ObserverContext) -> Result<(), ObserverError> {
        for record in &mut ctx.records {
            // Extract system metadata from record data (clean separation)
            record.extract_system_metadata();
            
            // Add computed fields to metadata (not record data)
            if let Some(created_at) = record.response_metadata.system.created_at {
                let age_days = (Utc::now() - created_at).num_days();
                record.add_computed_field("age_days", Value::Number(age_days.into()), "MetadataEnricher");
            }
            
            // Add display names
            if let Some(user_id) = record.get_field("user_id") {
                if let Some(user_id_str) = user_id.as_str() {
                    let display_name = self.get_user_display_name(&ctx.system, user_id_str).await
                        .unwrap_or_else(|_| "Unknown User".to_string());
                    
                    record.add_computed_field("user_display_name", Value::String(display_name), "MetadataEnricher");
                }
            }
            
            // Set permission metadata
            let user = ctx.system.get_user();
            let permissions = self.calculate_permissions(&user, record);
            record.set_permissions(permissions);
            
            // Add relationship counts
            if ctx.schema_name == "organizations" {
                let user_count = self.count_related_users(&ctx.system, record.id.unwrap()).await.unwrap_or(0);
                record.add_relationship_count("users", user_count);
            }
        }
        
        tracing::info!("Enhanced {} records with metadata", ctx.records.len());
        Ok(())
    }
}

impl MetadataEnricher {
    fn calculate_permissions(&self, user: &User, record: &StatefulRecord) -> PermissionMetadata {
        PermissionMetadata {
            can_read: true, // They got the record, so they can read it
            can_edit: self.user_can_edit_record(user, record),
            can_delete: self.user_can_delete_record(user, record),
            can_share: self.user_can_share_record(user, record),
            effective_access_level: self.calculate_access_level(user, record),
            permission_source: self.get_permission_source(user, record),
        }
    }
    
    // ... other helper methods
}
```

### 5. Route Handler Integration

```rust
// src/handlers/protected/data/find.rs
use axum::extract::Query;

#[derive(Debug, Deserialize)]
pub struct FindQuery {
    #[serde(flatten)]
    pub filter_params: HashMap<String, String>,
    
    /// Metadata inclusion options
    /// Examples: ?metadata=true, ?metadata=system,permissions, ?metadata=system.created_at
    pub metadata: Option<String>,
}

pub async fn find_post(
    Path(schema): Path<String>,
    Extension(ctx): Extension<RequestContext>,
    Query(query): Query<FindQuery>,
    Json(filter_data): Json<FilterData>,
) -> Result<ApiResponse<Vec<serde_json::Value>>, ApiError> {
    
    // Parse metadata options from query parameter
    let metadata_options = MetadataOptions::from_query_param(query.metadata.as_deref());
    
    // Execute SELECT operation through observer pipeline
    let pipeline = ObserverPipeline::new();
    let schema_obj = ctx.system.load_schema(&schema).await?;
    
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
    
    // Convert StatefulRecords to API response with metadata options
    let results: Vec<serde_json::Value> = observer_result.records.into_iter()
        .map(|record| record.to_api_response(&metadata_options))
        .collect();
    
    Ok(ApiResponse::success(results))
}
```

## Usage Examples

### Basic Query (No Metadata)
```http
GET /api/data/users
```
```json
[
  {
    "data": {
      "id": "123",
      "name": "John Smith",
      "email": "john@example.com"
    }
  }
]
```

### All Metadata
```http
GET /api/data/users?metadata=true
```
```json
[
  {
    "data": {
      "id": "123", 
      "name": "John Smith",
      "email": "john@example.com"
    },
    "metadata": {
      "system": {
        "created_at": "2024-01-01T00:00:00Z",
        "updated_at": "2024-01-15T10:30:00Z",
        "access_read": ["uuid1", "uuid2"]
      },
      "computed": {
        "age_days": 45,
        "user_display_name": "John Smith"
      },
      "permissions": {
        "can_read": true,
        "can_edit": true,
        "can_delete": false,
        "effective_access_level": "Edit"
      },
      "relationships": {
        "related_counts": {"projects": 5, "tasks": 23}
      }
    }
  }
]
```

### Selective Metadata
```http
GET /api/data/users?metadata=system,permissions
```
```json
[
  {
    "data": {...},
    "metadata": {
      "system": {...},
      "permissions": {...}
    }
  }
]
```

### Specific Fields Only
```http
GET /api/data/users?metadata=system.created_at,permissions.can_edit
```
```json
[
  {
    "data": {...},
    "metadata": {
      "system": {
        "created_at": "2024-01-01T00:00:00Z"
      },
      "permissions": {
        "can_edit": true
      }
    }
  }
]
```

## Benefits of This Approach

### 1. **Clean Data Separation**
- User data stays pure, no system field pollution
- Clear distinction between data and metadata
- No namespace conflicts

### 2. **Granular Control** 
- Clients can request exactly the metadata they need
- Reduces response size for simple queries
- Enables rich UIs when metadata is needed

### 3. **Extensible**
- Easy to add new metadata categories
- Observer-based enrichment keeps it modular
- No breaking changes when adding metadata

### 4. **Performance Optimized**
- Only compute requested metadata
- Can cache expensive metadata computations
- Minimal overhead for simple queries

### 5. **API Consistency**
- Same pattern works for all operations (CREATE/UPDATE/SELECT)
- Predictable response structure
- Easy to document and consume

This metadata system transforms records from simple data containers into rich, contextual objects while keeping the API clean and performant.