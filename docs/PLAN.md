# Monk API Rust - Implementation Plan

## Project Overview
A high-performance, type-safe reimplementation of monk-api in Rust, maintaining 100% compatibility with monk-cli while leveraging Rust's superior safety, performance, and concurrency features.

## Architecture Summary

### Core Components
1. **Dual-Database System**: `monk_main` (system) + `tenant_<hash>` (isolated tenant databases)
2. **Observer Pipeline**: 10-ring processing system for all database operations
3. **Filter Language**: 25+ operators for complex queries compatible with PostgreSQL arrays
4. **Stateful Records**: Change-tracking system for efficient database operations
5. **Transaction Management**: Automatic BEGIN/COMMIT/ROLLBACK at API boundaries
6. **Metadata System**: Clean separation of data and enrichment metadata

## Implementation Phases

### Phase 1: Foundation (Week 1)
**Goal**: Core infrastructure and database connectivity

#### 1.1 Database Layer
```
src/database/
├── manager.rs        # Connection pooling, multi-database management
├── models/           # Core schema types (User, Tenant, Schema)
│   ├── user.rs
│   ├── tenant.rs
│   └── schema.rs
├── error.rs          # Database error types
└── mod.rs
```
- Implement `DatabaseManager` for monk_main + tenant_* connections
- Create SQLx-based models with `FromRow` derivations
- Set up connection pooling with lazy initialization

#### 1.2 Configuration & Environment
```
src/config/
├── mod.rs            # Configuration management
└── env.rs            # Environment variable handling
```
- Load DATABASE_URL and JWT secrets
- Parse service configuration (ports, timeouts)
- Validate required environment variables

### Phase 2: Authentication & Security (Week 1-2)
**Goal**: JWT-based multi-tenant authentication

#### 2.1 Authentication System
```
src/auth/
├── jwt.rs            # JWT generation/validation
├── claims.rs         # JWT claims structure
├── middleware.rs     # Authentication middleware
└── mod.rs
```
- Implement tenant resolution (name → database)
- Create JWT with tenant context embedding
- Build authentication middleware for protected routes

#### 2.2 Request Context
```
src/api/
├── context.rs        # RequestContext with transaction support
├── response.rs       # ApiResponse types (100% TypeScript compatible)
├── error.rs          # ApiError with proper error codes
└── middleware/
    └── transaction.rs # Transaction middleware
```
- Implement `RequestContext` with optional transaction
- Create response types matching TypeScript exactly
- Build transaction middleware for write operations

### Phase 3: Filter Language (Week 2)
**Goal**: Complex query capabilities with 25+ operators

#### 3.1 Filter System
```
src/filter/
├── types.rs          # FilterOp enum, FilterData struct
├── filter.rs         # Main Filter class
├── filter_where.rs   # WHERE clause generation
├── filter_order.rs   # ORDER BY generation
└── error.rs          # Filter error types
```
- Port all 25+ operators from TypeScript
- Implement PostgreSQL array operations ($any, $all)
- Build parameterized query generation

#### 3.2 Query Builder Integration
```
src/database/
├── query_builder.rs  # Type-safe query construction
└── repository.rs     # High-level database operations
```
- Create `QueryBuilder<T>` with Filter integration
- Implement `Repository<T>` matching TypeScript Database class methods
- Add `DynamicRepository` for runtime schemas

### Phase 4: Observer System (Week 2-3)
**Goal**: 10-ring processing pipeline with compile-time safety

#### 4.1 Core Observer Infrastructure
```
src/observer/
├── traits.rs         # Ring-specific observer traits
├── context.rs        # ObserverContext with typed metadata
├── pipeline.rs       # ObserverPipeline execution engine
├── stateful_record.rs # StatefulRecord with change tracking
└── error.rs          # Observer error types
```
- Define traits for each ring (DataPreparation, Validation, etc.)
- Implement `StatefulRecord` with diff calculation
- Build pipeline execution with selective ring processing

#### 4.2 Observer Implementations
```
src/observer/implementations/
├── record_preloader.rs       # Ring 0: Load existing records
├── json_schema_validator.rs  # Ring 1: Schema validation
├── soft_delete_protector.rs  # Ring 2: Security checks
├── timestamp_enricher.rs     # Ring 4: Add timestamps
├── sql_operations.rs         # Ring 5: Database execution
├── result_enricher.rs        # Ring 6: Post-processing
├── query_access_control.rs   # Ring 2: ACL for SELECT
└── audit_logger.rs          # Ring 7: Audit trail
```
- Port all TypeScript observers to Rust
- Add compile-time observer registration
- Implement type-safe metadata passing

### Phase 5: API Routes (Week 3-4)
**Goal**: RESTful API with TypeScript-compatible responses

#### 5.1 Route Handlers
```
src/handlers/
├── public/
│   └── auth/
│       └── login.rs  # Tenant + user authentication
├── protected/
│   ├── data/         # CRUD operations
│   │   ├── find.rs   # POST /api/find/:schema
│   │   ├── schema_post.rs  # POST /api/data/:schema
│   │   ├── schema_put.rs   # PUT /api/data/:schema
│   │   └── schema_delete.rs # DELETE /api/data/:schema
│   └── meta/         # Schema management
│       └── schema_post.rs   # POST /api/meta/:schema
└── root/
    └── tenant/       # Cross-tenant operations
```
- Implement all monk-api endpoints
- Integrate observer pipeline with handlers
- Ensure response format compatibility

#### 5.2 Router Configuration
```
src/main.rs
```
- Set up Axum router with middleware layers
- Apply transaction middleware to write routes
- Configure JWT authentication for protected routes

### Phase 6: Dynamic Schema System (Week 4)
**Goal**: JSON Schema to PostgreSQL DDL generation

#### 6.1 Schema Management
```
src/schema/
├── validator.rs      # JSON Schema validation
├── generator.rs      # PostgreSQL DDL generation
├── registry.rs       # Schema registry operations
└── types.rs          # Schema-related types
```
- Validate JSON Schema definitions
- Generate CREATE TABLE statements
- Manage schema lifecycle (pending → active)

### Phase 7: Testing & Validation (Week 4-5)
**Goal**: Comprehensive test coverage

#### 7.1 Test Infrastructure
```
tests/
├── integration/
│   ├── auth_test.rs
│   ├── crud_test.rs
│   ├── filter_test.rs
│   └── observer_test.rs
└── fixtures/
    └── test_data.sql
```
- Unit tests for each component
- Integration tests for API endpoints
- Performance benchmarks vs TypeScript

### Phase 8: Production Readiness (Week 5)
**Goal**: Deployment preparation

#### 8.1 Operations
- Add comprehensive logging with tracing
- Implement health checks and metrics
- Create Docker deployment configuration
- Write migration scripts from TypeScript

## Key Implementation Details

### 1. Transaction Pattern
```rust
// Middleware automatically handles transactions
pub async fn with_transaction(request: Request, next: Next) -> Response {
    let tx = pool.begin().await?;
    let response = next.run(request).await;
    
    match response.status() {
        success => tx.commit().await,
        _ => tx.rollback().await
    }
}
```

### 2. Observer Registration
```rust
// Compile-time observer registration
impl ObserverPipeline {
    pub fn new() -> Self {
        Self {
            ring_0: vec![
                Box::new(RecordPreloader::default()),
                Box::new(UpdateMerger::default()),
            ],
            // ... other rings
        }
    }
}
```

### 3. Filter Integration
```rust
// Complex query support
let filter = FilterData {
    where_clause: json!({
        "$and": [
            { "status": "active" },
            { "access_read": { "$any": [user_id] } }
        ]
    })
};
```

### 4. Response Compatibility
```rust
// 100% TypeScript compatible
#[derive(Serialize)]
pub struct ApiSuccessResponse<T> {
    pub success: bool,  // Always true
    pub data: T,
}

#[derive(Serialize)]
pub struct ApiErrorResponse {
    pub success: bool,  // Always false
    pub error: String,
    pub error_code: String,  // "NOT_FOUND", "VALIDATION_ERROR", etc.
}
```

## Success Criteria

1. **API Compatibility**: All monk-cli operations work without modification
2. **Response Format**: Identical JSON structure to TypeScript version
3. **Performance**: 2-5x faster than Node.js implementation
4. **Type Safety**: Zero runtime type errors
5. **Transaction Safety**: Automatic rollback on any error
6. **Test Coverage**: >80% code coverage

## Dependencies

### Core
- **axum**: Web framework
- **sqlx**: Database access with compile-time verification
- **tokio**: Async runtime
- **serde/serde_json**: JSON serialization

### Security
- **jsonwebtoken**: JWT handling
- **argon2**: Password hashing

### Utilities
- **thiserror**: Error handling
- **tracing**: Logging
- **uuid**: UUID generation
- **chrono**: DateTime handling

## Deployment

### Development
```bash
cargo run --features runtime-env
```

### Production
```bash
cargo build --release
./target/release/monk-api-rust
```

### Docker
```dockerfile
FROM rust:1.75-slim
WORKDIR /app
COPY . .
RUN cargo build --release
CMD ["./target/release/monk-api-rust"]
```

## Migration Strategy

1. **Phase 1**: Deploy Rust API alongside TypeScript
2. **Phase 2**: Route read traffic to Rust
3. **Phase 3**: Route write traffic to Rust
4. **Phase 4**: Decommission TypeScript version

## Risk Mitigation

1. **API Compatibility**: Extensive integration testing against monk-cli
2. **Data Migration**: No database schema changes required
3. **Rollback Plan**: Can instantly switch back to TypeScript version
4. **Performance**: Load testing before production deployment

This plan provides a clear path to building a production-ready Rust implementation that maintains full compatibility while delivering superior performance, type safety, and maintainability.