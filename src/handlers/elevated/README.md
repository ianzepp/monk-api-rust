# Elevated Handlers

**Security Tier 3: Root JWT Authentication Required**

Elevated endpoints that require root-level JWT tokens. These provide administrative operations that span multiple tenants or system-level management.

## Route Prefix  
All elevated handlers map to routes with `/api/root` prefix:
- Routes: `/api/root/*`
- **Root JWT token required** (obtained via `POST /api/auth/sudo`)
- Administrative/system-level permissions  
- Cross-tenant operations allowed

## Middleware Stack
```rust
.layer(root_access_middleware())     // Validates root JWT token
.layer(jwt_auth_middleware())        // Base JWT validation
.layer(user_validation_middleware()) // User context (admin)
.layer(system_context_middleware())  // System dependencies
.layer(response_json_middleware())   // JSON responses  
```

## Security Flow
1. User authenticates normally → gets standard JWT
2. User calls `POST /api/auth/sudo` → gets **elevated root JWT**  
3. User can access `/api/root/*` endpoints with root JWT
4. Root JWT typically has shorter expiration time

## Handlers

### Root Operations (`/api/root/*`)
System administrative operations:

#### Tenant Management (`/api/root/tenant/*`)
Multi-tenant administration operations:

- **POST /api/root/tenant** → `root/tenant/create.rs`
  - Create new tenant with database provisioning
  - Input: `{ "name": "string", "display_name": "string" }`
  
- **GET /api/root/tenant** → `root/tenant/list.rs`  
  - List all tenants in system with health status
  - Supports pagination and filtering

- **GET /api/root/tenant/:name** → `root/tenant/show.rs`
  - Get detailed tenant information and metrics
  - Returns: tenant config, database info, user count, etc.

- **PATCH /api/root/tenant/:name** → `root/tenant/update.rs`
  - Update tenant configuration and settings
  - Input: `{ "display_name": "string", "settings": {...} }`

- **DELETE /api/root/tenant/:name** → `root/tenant/delete.rs`  
  - Soft delete tenant (preserves data)
  - Marks tenant as deleted but keeps database

- **PUT /api/root/tenant/:name** → `root/tenant/restore.rs`
  - Restore soft-deleted tenant  
  - Re-enables tenant and restores access

- **GET /api/root/tenant/:name/health** → `root/tenant/health.rs`
  - Check tenant database health and connectivity
  - Returns: database status, table counts, size metrics

## TypeScript Equivalent
```typescript  
// monk-api/src/routes/root/
import { rootRouter } from '@src/routes/root/index.js';

// Tenant management
rootRouter.post('/tenant', tenantPOST);
rootRouter.get('/tenant', tenantGET);  
rootRouter.get('/tenant/:name', tenantShowGET);
// ... etc
```

## Security Implications

- **Audit Logging**: All root operations are logged for security audit
- **Elevated Token Tracking**: Root JWTs track original user and elevation reason
- **Limited Duration**: Root tokens expire faster than standard JWTs  
- **IP Restrictions**: May be restricted to admin IP ranges (future)
- **MFA Required**: May require multi-factor authentication (future)

## Development vs Production

- **Development**: Root endpoints may be available on localhost without full auth
- **Production**: Always requires full sudo elevation process
- **Environment Variables**: Controls availability and restrictions

## Risk Management

These endpoints can:
- Create/delete entire tenant databases
- Access data across all tenants  
- Modify system-wide configurations
- Affect platform stability

**Use with extreme caution and comprehensive logging.**