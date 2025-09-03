// handlers/public/mod.rs - Public handlers (no authentication required)
// 
// This module contains all endpoints that do not require JWT authentication.
// These are primarily used for token acquisition and public documentation.
// 
// Security Level: None (completely public access)
// Route Prefix: No /api prefix (e.g., /auth/*, /docs/*)
// Middleware: None (no authentication or authorization)

// Public authentication module for token acquisition
pub mod auth;

// Re-export auth handlers for easy importing  
pub use auth::*;

/*
PUBLIC HANDLER ARCHITECTURE:

This module mirrors the monk-api TypeScript public handlers:
- TypeScript: src/public/auth/routes.ts  
- Rust:       src/handlers/public/auth/mod.rs

Key Differences from Protected Handlers:
1. **No JWT Required**: Completely anonymous access
2. **No User Context**: Handlers don't receive authenticated user  
3. **Input Validation**: Must validate all inputs (no trusted user context)
4. **Rate Limiting**: Should implement to prevent abuse
5. **Security Logging**: Extra logging for authentication attempts

Usage Pattern:
```rust
use handlers::public::auth;

Router::new()
    .route("/auth/login", post(auth::login))
    .route("/auth/register", post(auth::register))
    .route("/auth/refresh", post(auth::refresh))
    // No middleware layers - completely public
```

These handlers are the entry point for all user authentication flows.
*/