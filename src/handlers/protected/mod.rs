// handlers/protected/mod.rs - Protected handlers (JWT authentication required)
//
// This module contains all endpoints that require valid JWT authentication.
// These handlers provide the core API functionality for authenticated users.
//
// Security Level: JWT Authentication Required
// Route Prefix: /api/* (e.g., /api/auth/*, /api/data/*, /api/meta/*)  
// Middleware: JWT validation + user context + system dependencies

// Protected module declarations
pub mod auth;  // User account management endpoints
pub mod data;   // Dynamic data CRUD operations  
pub mod meta;   // JSON Schema management endpoints
pub mod find;   // Advanced filtered finds

// Re-export all handler functions for easy importing
pub use auth::*;
pub use data::*; 
pub use meta::*;

/*
PROTECTED HANDLER ARCHITECTURE:

This module mirrors the monk-api TypeScript protected handlers:
- TypeScript: src/routes/ (auth, data, meta subdirectories)
- Rust:       src/handlers/protected/ (auth, data, meta subdirectories)

Middleware Stack Applied to All Protected Routes:
```rust
Router::new()
    .route("/api/auth/whoami", get(protected::auth::whoami_get))
    .route("/api/data/:schema", get(protected::data::schema_get))
    .route("/api/meta/:schema", get(protected::meta::schema_get))
    .layer(jwt_auth_middleware())        // Validates JWT token
    .layer(user_validation_middleware()) // Loads user context
    .layer(system_context_middleware())  // Injects dependencies
    .layer(response_json_middleware())   // Ensures JSON responses
```

Handler Context:
Each protected handler receives:
- **Validated JWT Token**: User claims and permissions
- **User Object**: Full user details loaded from database
- **Tenant Context**: Isolated database connection for tenant
- **System Dependencies**: Database pool, cache, logging, etc.

Security Model:
- **Tenant Isolation**: Users can only access their tenant's data
- **Schema Permissions**: Users must have appropriate schema access
- **Audit Logging**: All operations logged for security audit
- **Rate Limiting**: API rate limits applied per user/tenant

Handler Categories:

1. **Auth Handlers** (/api/auth/):
   - User account management within authenticated context
   - Permission elevation (sudo operations)
   - Session management and user profile operations

2. **Data Handlers** (/api/data/):
   - CRUD operations on dynamic tenant schemas
   - Bulk operations for efficiency
   - Query filtering, pagination, and sorting

3. **Meta Handlers** (/api/meta/):
   - JSON Schema definition management
   - Automatic PostgreSQL table generation
   - Schema versioning and migration support

This tier provides the core business logic and data management functionality
that authenticated users interact with in their day-to-day operations.
*/