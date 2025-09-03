# Handlers Architecture

This directory contains the complete 3-tier handler architecture that mirrors the monk-api TypeScript security model.

## Security Architecture Overview

The monk-api-rust uses a progressive security model with three distinct tiers:

```
Public → Protected → Elevated
  ↓         ↓          ↓
No Auth   JWT Auth   Root JWT
```

## Directory Structure

```
handlers/
├── README.md              ← This file
├── mod.rs                 ← Module declarations and re-exports
├── public/                ← Tier 1: No authentication required
│   ├── README.md          ← Public endpoints documentation  
│   ├── mod.rs             ← Public module declarations
│   └── auth/              ← Token acquisition endpoints
│       ├── README.md      ← Auth flow documentation
│       ├── mod.rs         ← Auth handler exports
│       ├── login.rs       ← POST /auth/login
│       ├── register.rs    ← POST /auth/register
│       └── refresh.rs     ← POST /auth/refresh
├── protected/             ← Tier 2: JWT authentication required
│   ├── README.md          ← Protected API documentation
│   ├── mod.rs             ← Protected module declarations  
│   ├── auth/              ← User account management
│   │   ├── README.md      ← Auth operations documentation
│   │   ├── mod.rs         ← Auth handler exports
│   │   ├── whoami.rs      ← GET /api/auth/whoami
│   │   └── sudo.rs        ← POST /api/auth/sudo (elevation)
│   ├── data/              ← Dynamic data operations
│   │   ├── README.md      ← Data API documentation
│   │   └── ...
│   └── meta/              ← Schema management
│       ├── README.md      ← Schema API documentation  
│       └── ...
└── elevated/              ← Tier 3: Root JWT required (sudo first)
    ├── README.md          ← Administrative operations documentation
    ├── mod.rs             ← Elevated module declarations
    └── root/              ← Root administrative operations
        ├── README.md      ← Root API documentation
        ├── mod.rs         ← Root handler exports
        └── tenant/        ← Multi-tenant management
            ├── README.md  ← Tenant management documentation
            └── ...
```

## Security Flow

### 1. Public Endpoints (`/auth/*`)
- **No authentication required**
- Used to obtain initial JWT tokens
- Available to anonymous users
- Routes: `/auth/login`, `/auth/register`, `/auth/refresh`

### 2. Protected Endpoints (`/api/*`)  
- **JWT authentication required**
- Standard API operations for authenticated users
- JWT middleware validates token on all `/api/*` routes
- Routes: `/api/auth/whoami`, `/api/data/:schema`, `/api/meta/:schema`

### 3. Elevated Endpoints (`/api/root/*`)
- **Root-level JWT required**
- Must call `POST /api/auth/sudo` first to elevate privileges
- Administrative operations for tenant management
- Root access middleware validates elevated JWT token
- Routes: `/api/root/tenant/*`

## Middleware Architecture

```rust
// Public routes - no middleware
Router::new()
    .route("/auth/login", post(public::auth::login))
    
// Protected routes - JWT middleware
Router::new()  
    .route("/api/auth/whoami", get(protected::auth::whoami))
    .layer(jwt_auth_middleware())
    
// Elevated routes - JWT + Root middleware  
Router::new()
    .route("/api/root/tenant", post(elevated::root::tenant::create))
    .layer(root_access_middleware())
    .layer(jwt_auth_middleware())
```

## TypeScript Equivalents

This structure directly maps to the monk-api TypeScript architecture:

| Rust                  | TypeScript              | Security Level |
|-----------------------|-------------------------|----------------|
| `handlers/public/`    | `src/public/`           | None           |
| `handlers/protected/` | `src/routes/`           | JWT            |
| `handlers/elevated/`  | `src/routes/root/`      | Root JWT       |

## Development Notes

- Each tier has its own `README.md` for context-specific documentation
- Handler functions use verbose comments for learning purposes
- All endpoints return descriptive 501 responses during development
- Structure allows for independent testing of security tiers
- AI tools can quickly understand purpose and security model from README files

## Next Steps

1. Implement JWT middleware for protected tier
2. Implement root access middleware for elevated tier  
3. Add database connections and business logic
4. Create comprehensive tests for each security tier