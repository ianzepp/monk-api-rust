# Database Architecture - Monk API Rust

This document describes the two-database architecture used by monk-api and how to integrate it with the Rust implementation.

## Overview

Monk API uses a **dual-database architecture** to provide complete tenant isolation while maintaining system-wide management capabilities:

1. **`monk_main`** - System database for tenant registry and global operations
2. **`tenant_<hash>`** - Individual databases per tenant for complete data isolation

## Database Structure

### System Database (`monk_main`)

The central registry database containing:

#### `tenants` Table
Manages tenant configuration and database routing:

```sql
CREATE TABLE tenants (
    id           UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    name         VARCHAR(255) UNIQUE NOT NULL,           -- Login identifier
    database     VARCHAR(255) NOT NULL,                  -- Database name
    host         VARCHAR(255) DEFAULT 'localhost',       -- Database host
    is_active    BOOLEAN DEFAULT true,                   -- Enable/disable tenant
    tenant_type  VARCHAR(20) DEFAULT 'normal',           -- 'normal' or 'template'
    access_read  UUID[] DEFAULT '{}',                    -- Read access control
    access_edit  UUID[] DEFAULT '{}',                    -- Edit access control  
    access_full  UUID[] DEFAULT '{}',                    -- Full access control
    access_deny  UUID[] DEFAULT '{}',                    -- Deny access control
    created_at   TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at   TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    trashed_at   TIMESTAMP,                              -- Soft delete
    deleted_at   TIMESTAMP                               -- Hard delete
);

-- Key indexes
CREATE UNIQUE INDEX tenants_name_key ON tenants (name);
CREATE INDEX idx_tenants_database ON tenants (database);
CREATE INDEX idx_tenants_name_active ON tenants (name, is_active);
```

**Key Fields:**
- `name`: Unique tenant identifier used in authentication (e.g., "acme_corp")
- `database`: PostgreSQL database name (e.g., "tenant_007314608dd04169")
- `host`: Database host for future distributed deployment
- `tenant_type`: 'normal' (regular tenant) or 'template' (fixture for cloning)

#### `requests` Table
Global request logging for analytics and monitoring:

```sql
CREATE TABLE requests (
    id          UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    timestamp   TIMESTAMP NOT NULL DEFAULT now(),
    method      VARCHAR(10) NOT NULL,                    -- HTTP method
    url         TEXT NOT NULL,                          -- Full URL
    path        TEXT NOT NULL,                          -- Path component
    api         VARCHAR(20),                            -- API category extracted from path
    ip_address  INET,                                   -- Client IP address
    user_agent  TEXT,                                   -- HTTP User-Agent header
    created_at  TIMESTAMP NOT NULL DEFAULT now(),
    updated_at  TIMESTAMP NOT NULL DEFAULT now()
);

CREATE INDEX idx_requests_timestamp ON requests (timestamp);
```

### Tenant Databases (`tenant_<hash>`)

Each tenant has a dedicated PostgreSQL database with complete schema isolation. Database names use 16-character hex hashes (e.g., `tenant_007314608dd04169`).

#### Core Tables (Present in Every Tenant Database)

##### `users` Table
Tenant-specific user authentication and authorization:

```sql
CREATE TABLE users (
    id          UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    name        TEXT NOT NULL,                          -- Display name
    auth        TEXT UNIQUE NOT NULL,                   -- Login username
    access      TEXT NOT NULL CHECK (access IN ('root', 'full', 'edit', 'read', 'deny')),
    access_read UUID[] DEFAULT '{}',                    -- Schema-level read permissions
    access_edit UUID[] DEFAULT '{}',                    -- Schema-level edit permissions
    access_full UUID[] DEFAULT '{}',                    -- Schema-level full permissions  
    access_deny UUID[] DEFAULT '{}',                    -- Schema-level deny permissions
    created_at  TIMESTAMP NOT NULL DEFAULT now(),
    updated_at  TIMESTAMP NOT NULL DEFAULT now(),
    trashed_at  TIMESTAMP,                              -- Soft delete
    deleted_at  TIMESTAMP                               -- Hard delete
);

CREATE UNIQUE CONSTRAINT users_auth_unique ON users (auth);
```

**Access Levels:**
- `root`: System administrator (can manage schemas and users)
- `full`: Full data access (read/write all schemas) 
- `edit`: Edit access to assigned schemas
- `read`: Read-only access to assigned schemas
- `deny`: Explicitly denied access

##### `schemas` Table
Dynamic schema management - maps JSON Schema definitions to PostgreSQL tables:

```sql
CREATE TABLE schemas (
    id            UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    name          TEXT UNIQUE NOT NULL,                 -- Schema name (e.g., "products")
    table_name    TEXT UNIQUE NOT NULL,                 -- PostgreSQL table name
    status        TEXT NOT NULL DEFAULT 'pending',     -- 'pending', 'active', 'system'
    definition    JSONB NOT NULL,                       -- Original JSON Schema definition
    field_count   TEXT NOT NULL,                        -- Number of fields generated
    json_checksum TEXT,                                 -- Schema version control
    access_read   UUID[] DEFAULT '{}',                  -- Schema-specific read access
    access_edit   UUID[] DEFAULT '{}',                  -- Schema-specific edit access
    access_full   UUID[] DEFAULT '{}',                  -- Schema-specific full access
    access_deny   UUID[] DEFAULT '{}',                  -- Schema-specific deny access
    created_at    TIMESTAMP NOT NULL DEFAULT now(),
    updated_at    TIMESTAMP NOT NULL DEFAULT now(),
    trashed_at    TIMESTAMP,                            -- Soft delete
    deleted_at    TIMESTAMP                             -- Hard delete
);

CREATE UNIQUE CONSTRAINT schema_name_unique ON schemas (name);
CREATE UNIQUE CONSTRAINT schema_table_name_unique ON schemas (table_name);
```

**Schema Status:**
- `pending`: Schema definition created but table not yet generated
- `active`: PostgreSQL table created and available for data operations
- `system`: Core system tables (users, schemas) - not user-modifiable

##### `columns` Table
Metadata about dynamically generated table columns:

```sql
CREATE TABLE columns (
    schema_name TEXT REFERENCES schemas(name),
    -- Additional column metadata fields
);
```

#### Dynamic Tables
Based on JSON Schema definitions, monk-api automatically generates PostgreSQL tables with:

- Standard fields: `id` (UUID), `access_*` arrays, `created_at`, `updated_at`, `trashed_at`, `deleted_at`
- Schema-defined fields: Based on JSON Schema properties
- Proper constraints, indexes, and data types

**Example Dynamic Table (`account`):**
```sql
CREATE TABLE account (
    id           UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    access_read  UUID[] DEFAULT '{}',
    access_edit  UUID[] DEFAULT '{}', 
    access_full  UUID[] DEFAULT '{}',
    access_deny  UUID[] DEFAULT '{}',
    created_at   TIMESTAMP NOT NULL DEFAULT now(),
    updated_at   TIMESTAMP NOT NULL DEFAULT now(),
    trashed_at   TIMESTAMP,
    deleted_at   TIMESTAMP,
    -- Schema-specific fields:
    name         TEXT,
    email        TEXT,
    username     TEXT,
    account_type TEXT,
    balance      NUMERIC,
    is_active    BOOLEAN,
    is_verified  BOOLEAN,
    credit_limit NUMERIC,
    last_login   TIMESTAMP,
    preferences  JSONB,
    metadata     JSONB,
    phone        TEXT
);
```

## Database Integration Patterns

### Connection Management

The Rust implementation needs to handle connections to multiple databases:

```rust
// System database connection (monk_main)
let system_db = PgPool::connect("postgresql://user@localhost/monk_main").await?;

// Tenant database connection (resolved at runtime)
let tenant_db = PgPool::connect(&format!(
    "postgresql://user@localhost/{}", 
    tenant_database_name
)).await?;
```

### Authentication Flow

1. **Login Request** (`POST /auth/login`):
   ```rust
   // 1. Query monk_main.tenants to find tenant database
   let tenant = sqlx::query!("SELECT database FROM tenants WHERE name = $1 AND is_active = true", tenant_name)
       .fetch_one(&system_db).await?;
   
   // 2. Connect to tenant database
   let tenant_db = get_tenant_connection(&tenant.database).await?;
   
   // 3. Authenticate user in tenant database
   let user = sqlx::query!("SELECT * FROM users WHERE auth = $1", username)
       .fetch_one(&tenant_db).await?;
   
   // 4. Generate JWT with tenant database name
   let claims = JwtClaims {
       user_id: user.id,
       tenant_name: tenant_name.to_string(),
       tenant_database: tenant.database,
       access_level: user.access,
   };
   ```

2. **Protected Request** (requires JWT):
   ```rust
   // Extract tenant database from JWT claims
   let tenant_db = get_tenant_connection(&claims.tenant_database).await?;
   
   // Execute operation in tenant's isolated database
   let results = sqlx::query!("SELECT * FROM account WHERE access_read @> $1", user_permissions)
       .fetch_all(&tenant_db).await?;
   ```

### Dynamic Schema Operations

1. **Create Schema** (`POST /api/meta/:schema`):
   ```rust
   // 1. Validate JSON Schema definition
   let schema_def: serde_json::Value = serde_json::from_str(&yaml_input)?;
   validate_json_schema(&schema_def)?;
   
   // 2. Generate PostgreSQL DDL from JSON Schema
   let ddl = generate_table_ddl(&schema_name, &schema_def)?;
   
   // 3. Execute DDL in tenant database  
   sqlx::query(&ddl).execute(&tenant_db).await?;
   
   // 4. Store schema definition in schemas table
   sqlx::query!("INSERT INTO schemas (name, table_name, definition, status) VALUES ($1, $2, $3, 'active')",
       schema_name, schema_name, schema_def
   ).execute(&tenant_db).await?;
   ```

2. **Data Operations** (`/api/data/:schema`):
   ```rust
   // 1. Verify schema exists and user has permissions
   let schema = sqlx::query!("SELECT * FROM schemas WHERE name = $1 AND status = 'active'", schema_name)
       .fetch_one(&tenant_db).await?;
   
   // 2. Execute dynamic query on generated table
   let query = format!("SELECT * FROM {} WHERE access_read @> $1", schema_name);
   let results = sqlx::query(&query).bind(&user_permissions).fetch_all(&tenant_db).await?;
   ```

### Root Operations (Cross-Tenant)

Root-level operations in `/api/root/tenant/*` work across the system database:

```rust
// Create new tenant (requires root JWT)
pub async fn tenant_create(claims: RootJwtClaims) -> Result<Json<Value>, AppError> {
    // 1. Generate unique database name
    let db_name = format!("tenant_{}", generate_hash());
    
    // 2. Create PostgreSQL database
    sqlx::query(&format!("CREATE DATABASE {}", db_name))
        .execute(&system_db).await?;
    
    // 3. Run initial migrations on new tenant database
    let tenant_db = get_tenant_connection(&db_name).await?;
    run_tenant_migrations(&tenant_db).await?;
    
    // 4. Register tenant in system database
    sqlx::query!("INSERT INTO tenants (name, database) VALUES ($1, $2)", 
        tenant_name, db_name
    ).execute(&system_db).await?;
    
    Ok(Json(json!({"success": true, "database": db_name})))
}
```

## Security Considerations

### Tenant Isolation
- **Database-level isolation**: Each tenant has completely separate PostgreSQL database
- **No cross-tenant queries**: Impossible to accidentally access other tenant's data
- **Resource isolation**: Database-level resource limits and quotas

### Access Control
- **JWT-based authentication**: Tenant and user context embedded in token
- **Schema-level permissions**: Granular access control per data schema
- **UUID-based permissions**: Fine-grained access control using UUID arrays

### Connection Security
- **Connection pooling**: Efficient database connection management per tenant
- **Prepared statements**: Protection against SQL injection via SQLx compile-time verification
- **Audit logging**: All requests logged in `monk_main.requests` table

## Implementation Roadmap

1. **Database Connection Layer**:
   - System database pool for `monk_main`
   - Dynamic tenant database pools
   - Connection caching and lifecycle management

2. **Authentication Service**:
   - Tenant resolution (`name` → `database`)
   - User authentication within tenant database
   - JWT generation with tenant context

3. **Dynamic Schema Engine**:
   - JSON Schema validation
   - PostgreSQL DDL generation
   - Schema registry management

4. **Data Access Layer**:
   - Dynamic query generation
   - Permission-based filtering
   - CRUD operations on dynamic tables

5. **Root Administrative Operations**:
   - Tenant database provisioning
   - Cross-tenant management
   - System monitoring and health checks

This architecture provides the foundation for a highly scalable, secure multi-tenant platform with complete data isolation and dynamic schema capabilities.