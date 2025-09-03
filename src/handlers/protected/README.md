# Protected Handlers  

**Security Tier 2: JWT Authentication Required**

Protected endpoints that require valid JWT authentication. These provide the core API functionality for authenticated users.

## Route Prefix
All protected handlers map to routes with `/api` prefix:
- Routes: `/api/auth/*`, `/api/data/*`, `/api/meta/*`
- JWT token required in Authorization header
- Standard user permissions apply

## Middleware Stack
```rust
.layer(jwt_auth_middleware())        // Validates JWT token
.layer(user_validation_middleware()) // Loads user context  
.layer(system_context_middleware())  // Injects system dependencies
.layer(response_json_middleware())   // Ensures JSON responses
```

## Handlers

### User Management (`/api/auth/*`)
Account management for authenticated users:

- **GET /api/auth/whoami** → `auth/whoami.rs`
  - Get current authenticated user details
  - Returns: user profile, tenant, permissions

- **POST /api/auth/sudo** → `auth/sudo.rs`
  - Elevate permissions to root level
  - Returns: elevated JWT token for `/api/root/*` access

### Data Operations (`/api/data/*`)
Dynamic CRUD operations on tenant schemas:

- **GET /api/data/:schema** → `data/schema_get.rs`
  - List all records in schema with pagination/filtering
  
- **POST /api/data/:schema** → `data/schema_post.rs`  
  - Create new records (single or bulk)
  
- **PUT /api/data/:schema** → `data/schema_put.rs`
  - Bulk update records
  
- **DELETE /api/data/:schema** → `data/schema_delete.rs`
  - Bulk delete records

### Schema Management (`/api/meta/*`)  
JSON Schema definitions that generate PostgreSQL tables:

- **GET /api/meta/:schema** → `meta/schema_get.rs`
  - Get JSON Schema definition
  
- **POST /api/meta/:schema** → `meta/schema_post.rs`
  - Create new schema + generate PostgreSQL table
  
- **PUT /api/meta/:schema** → `meta/schema_put.rs`
  - Update schema + alter PostgreSQL table
  
- **DELETE /api/meta/:schema** → `meta/schema_delete.rs`
  - Soft delete schema

## TypeScript Equivalent
```typescript
// monk-api/src/routes/
import * as authRoutes from '@src/routes/auth/routes.js';
import * as dataRoutes from '@src/routes/data/routes.js';  
import * as metaRoutes from '@src/routes/meta/routes.js';
```

## Security Context

Each handler receives:
- **Validated JWT token** with user claims
- **User object** loaded from database  
- **Tenant context** for data isolation
- **Database connection** scoped to tenant
- **System dependencies** (logging, caching, etc.)

## Authorization Model

- **Tenant Isolation**: Users can only access their tenant's data
- **Schema Permissions**: Users must have schema-level permissions  
- **Record-Level**: Future feature for fine-grained access control
- **Admin Operations**: Some operations require admin role within tenant

This tier provides the core business logic and data management capabilities.