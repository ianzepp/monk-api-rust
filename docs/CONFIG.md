# Configuration System

The Monk API uses a centralized configuration system that provides environment-specific settings with runtime overrides via environment variables.

## Architecture

The configuration system (`src/config/mod.rs`) provides:
- Centralized feature flags and settings
- Environment-based defaults (development, staging, production)
- Runtime overrides via environment variables
- Type-safe configuration access throughout the codebase

## Environment Variables

### Naming Convention

Environment variables follow a hierarchical naming pattern:
- `{MODULE}_{SETTING}` where MODULE is the configuration category

Examples:
- `FILTER_ALLOW_RAW_SQL`
- `DATABASE_MAX_CONNECTIONS`
- `API_ENABLE_RATE_LIMITING`
- `SECURITY_REQUIRE_HTTPS`

### Configuration Categories

#### Filter Configuration
- `FILTER_ALLOW_RAW_SQL` (bool): Enable/disable raw SQL in WHERE clauses
- `FILTER_MAX_LIMIT` (int): Maximum rows returned per query
- `FILTER_MAX_NESTED_DEPTH` (int): Maximum depth for nested logical operators
- `FILTER_ENABLE_QUERY_CACHE` (bool): Enable query result caching
- `FILTER_DEBUG_LOGGING` (bool): Enable debug logging for filter operations

#### Database Configuration
- `DATABASE_MAX_CONNECTIONS` (int): Maximum database connection pool size
- `DATABASE_CONNECTION_TIMEOUT` (int): Connection timeout in seconds
- `DATABASE_ENABLE_QUERY_LOGGING` (bool): Log all database queries
- `DATABASE_ENABLE_SLOW_QUERY_WARNING` (bool): Warn on slow queries
- `DATABASE_SLOW_QUERY_THRESHOLD_MS` (int): Slow query threshold in milliseconds

#### API Configuration
- `API_ENABLE_RATE_LIMITING` (bool): Enable API rate limiting
- `API_RATE_LIMIT_REQUESTS` (int): Requests allowed per window
- `API_RATE_LIMIT_WINDOW_SECS` (int): Rate limit window in seconds
- `API_ENABLE_REQUEST_LOGGING` (bool): Log all API requests
- `API_ENABLE_RESPONSE_COMPRESSION` (bool): Enable gzip compression
- `API_MAX_REQUEST_SIZE_BYTES` (int): Maximum request body size

#### Security Configuration
- `SECURITY_ENABLE_CORS` (bool): Enable CORS headers
- `SECURITY_CORS_ORIGINS` (string): Comma-separated allowed origins
- `SECURITY_REQUIRE_HTTPS` (bool): Force HTTPS connections
- `SECURITY_ENABLE_AUDIT_LOGGING` (bool): Enable security audit logs
- `SECURITY_JWT_EXPIRY_HOURS` (int): JWT token expiry time

## Usage

### Accessing Configuration

```rust
use crate::config::CONFIG;

// Check if raw SQL is allowed
if CONFIG.filter.allow_raw_sql {
    // Process raw SQL
}

// Get max database connections
let max_connections = CONFIG.database.max_connections;

// Check environment
match CONFIG.environment {
    Environment::Production => {
        // Production-specific logic
    }
    _ => {}
}
```

### Using Helper Macros

```rust
// Check if running in development
if is_development!() {
    // Development-only code
}

// Check if running in production
if is_production!() {
    // Production-only code
}
```

## Environment-Specific Defaults

### Development
- Raw SQL: Enabled
- Query limits: High (1000)
- Rate limiting: Disabled
- Logging: Verbose
- HTTPS: Not required

### Staging
- Raw SQL: Disabled
- Query limits: Medium (500)
- Rate limiting: Enabled
- Logging: Moderate
- HTTPS: Required

### Production
- Raw SQL: Disabled
- Query limits: Low (100)
- Rate limiting: Strict
- Logging: Minimal
- HTTPS: Required

## Security Considerations

1. **Raw SQL**: Disabled by default in production. When enabled, all raw SQL queries are logged for audit purposes.

2. **Audit Logging**: When enabled, sensitive operations (like raw SQL usage) are logged with context.

3. **Environment Detection**: Defaults to development mode if `APP_ENV` is not set - explicitly set for production.

## Example .env Files

### Development
```env
APP_ENV=development
FILTER_ALLOW_RAW_SQL=true
FILTER_MAX_LIMIT=1000
DATABASE_ENABLE_QUERY_LOGGING=true
API_ENABLE_RATE_LIMITING=false
SECURITY_REQUIRE_HTTPS=false
```

### Production
```env
APP_ENV=production
FILTER_ALLOW_RAW_SQL=false
FILTER_MAX_LIMIT=100
DATABASE_ENABLE_QUERY_LOGGING=false
API_ENABLE_RATE_LIMITING=true
SECURITY_REQUIRE_HTTPS=true
SECURITY_ENABLE_AUDIT_LOGGING=true
```

## Testing Configuration

```bash
# Test with different environments
APP_ENV=development cargo run
APP_ENV=production cargo run

# Override specific settings
FILTER_ALLOW_RAW_SQL=true APP_ENV=production cargo run
```