# WARP.md

This file provides guidance to WARP (warp.dev) when working with code in this repository.

## Project Context

This is **monk-api-rust**, a Rust rewrite of the Monk API PaaS management platform. The project is currently **under active development** and **non-functional** - it serves as both a Rust learning project and eventual production replacement for the TypeScript monk-api.

**Current Status**: Basic Axum web server with placeholder routes. Authentication, database operations, and schema management are not yet implemented.

## Development Commands

### Build & Run
```bash
# Build the project
cargo build

# Build for production (optimized)
cargo build --release

# Run development server (defaults to port 3000)
cargo run

# Run with custom port
MONK_API_PORT=4000 cargo run
# or
PORT=4000 cargo run
```

### Testing
```bash
# Run all tests
cargo test

# Run specific test file
cargo test --test 10_auth
cargo test --test 32_data_api
cargo test --test 44_filter

# Run tests with output
cargo test -- --nocapture

# Run tests in integration mode
cargo test --test '*'
```

### Code Quality
```bash
# Format code (uses rustfmt.toml config)
cargo fmt

# Check formatting without applying
cargo fmt -- --check

# Run linter
cargo clippy

# Run linter with all targets
cargo clippy --all-targets --all-features

# Check compilation without building
cargo check
```

### Development Tools
```bash
# Watch for changes and rebuild
cargo watch -x run

# Watch and run tests
cargo watch -x test

# Generate documentation
cargo doc --open

# Check for unused dependencies
cargo machete
```

## Environment Configuration

### Required Environment Variables
- `DATABASE_URL`: PostgreSQL connection string (required but not yet functional)
- `MONK_TENANT_DB`: Tenant database name (required but not yet functional)

### Optional Environment Variables
```bash
# Server Configuration
MONK_API_PORT=3000          # or PORT=3000
APP_ENV=development         # development|staging|production

# Database Settings
DATABASE_MAX_CONNECTIONS=10
DATABASE_CONNECTION_TIMEOUT=30
DATABASE_ENABLE_QUERY_LOGGING=true
DATABASE_SLOW_QUERY_THRESHOLD_MS=100

# API Configuration  
API_ENABLE_RATE_LIMITING=false
API_RATE_LIMIT_REQUESTS=60
API_MAX_REQUEST_SIZE_BYTES=2097152

# Security Settings
SECURITY_ENABLE_CORS=true
SECURITY_CORS_ORIGINS=http://localhost:3000,http://localhost:5173
SECURITY_JWT_EXPIRY_HOURS=24

# Filter System
FILTER_ALLOW_RAW_SQL=false
FILTER_MAX_LIMIT=100
FILTER_MAX_NESTED_DEPTH=5
```

Copy `.env.example` to `.env` and customize as needed.

## Architecture Overview

### Module Structure
```
src/
├── main.rs              # Axum server setup and route configuration
├── api/                 # API utilities (response formatting)
├── config/              # Environment-based configuration system
├── database/            # Database layer (SQLx-based, not yet implemented)
│   ├── manager.rs       # Connection pooling and multi-database support
│   ├── models/          # Core data models (User, Tenant, Schema)
│   ├── query_builder.rs # Type-safe query construction
│   └── repository.rs    # High-level database operations
├── filter/              # Complex query system (25+ operators planned)
│   ├── filter.rs        # Main filter processing
│   ├── filter_where.rs  # WHERE clause generation
│   └── types.rs         # Filter operation types
├── handlers/            # Three-tier security model
│   ├── public/          # No auth required (/auth/*)
│   ├── protected/       # JWT auth required (/api/*)
│   └── elevated/        # Root JWT auth required (/api/root/*)
└── observer/            # 10-ring processing pipeline (planned)
    ├── pipeline.rs      # Observer execution engine
    └── stateful_record.rs # Change-tracking system
```

### Configuration System
The project uses a sophisticated configuration system (`src/config/mod.rs`) with:
- Environment-specific defaults (development, staging, production)
- Runtime overrides via environment variables
- Type-safe configuration access via global singleton
- Helper macros: `is_development!()` and `is_production!()`

### Multi-Tenant Architecture (Planned)
- **System Database**: `monk_main` for tenant/user management
- **Tenant Databases**: `tenant_<hash>` for isolated data storage
- **Observer Pipeline**: 10-ring processing system for all database operations
- **Filter Language**: 25+ operators for complex queries with PostgreSQL array support

### Route Structure
```
/ and /health              # Public endpoints
/auth/login/:tenant/:user  # Public authentication
/api/auth/*               # Protected user management
/api/data/:schema         # Protected CRUD operations
/api/data/:schema/:id     # Protected record operations
/api/find/:schema         # Protected search/filter operations
/api/meta/:schema         # Protected schema management
/api/root/*               # Elevated admin operations (planned)
```

## Development Workflow

### Testing Current Implementation
```bash
# Start server
cargo run

# Test basic endpoints
curl http://localhost:3000/
curl http://localhost:3000/health

# Test placeholder endpoints (returns 501 Not Implemented)
curl -X POST http://localhost:3000/api/auth/login
curl http://localhost:3000/api/data/users
curl http://localhost:3000/api/meta/schema/users
```

### Code Formatting Standards
- **Max width**: 100 characters (rustfmt.toml)
- **Imports**: Grouped and sorted (std, external, crate)
- **Comments**: Wrapped at 100 characters
- **Style**: Follow rustfmt defaults with project overrides

### API Compatibility Target
This project aims for **100% compatibility** with the existing TypeScript monk-api and monk-cli tools. All responses must match the exact JSON structure:

```rust
// Success response
{
  "success": true,
  "data": { ... }
}

// Error response  
{
  "success": false,
  "error": "Error message",
  "error_code": "ERROR_TYPE"
}
```

## Integration with monk-cli

Once functional, this API will be a drop-in replacement for the TypeScript monk-api:

```bash
# monk-cli commands that will work identically
monk server add rust-local localhost:3000
monk server use rust-local
monk auth login my-app admin
monk data select users
```

The Rust implementation uses identical PostgreSQL schemas for seamless database compatibility.

## Documentation References

- **Project Status**: See `README.md` for current implementation status
- **Architecture Details**: See `docs/PLAN.md` for comprehensive implementation roadmap
- **Configuration Guide**: See `docs/CONFIG.md` for detailed environment variable documentation
- **Original API**: [monk-cli](https://github.com/ianzepp/monk-cli) TypeScript implementation

## Notes for AI Assistants

- This is a **learning project** and **active rewrite** - expect significant architectural changes
- Most functionality is **planned but not implemented** - refer to `docs/PLAN.md` for implementation timeline
- When working with database-related code, remember that database connections are not yet functional
- All API endpoints currently return placeholder responses or 501 Not Implemented
- Focus on type safety, error handling, and maintaining compatibility with the TypeScript API structure
