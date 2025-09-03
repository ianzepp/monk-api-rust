// handlers/public/auth/mod.rs - Public authentication handlers
// 
// Token acquisition endpoints that do not require authentication.
// These handlers implement the core authentication flow for obtaining JWT tokens.

use axum::{http::StatusCode, response::Json};
use serde_json::{json, Value};

// Authentication handler modules
pub mod login;    // POST /auth/login - authenticate and get JWT
pub mod register; // POST /auth/register - create new account  
pub mod refresh;  // POST /auth/refresh - refresh expired JWT

// Re-export handler functions with descriptive names
pub use login::login_post;       // Authenticate user credentials
pub use register::register_post; // Register new user account
pub use refresh::refresh_post;   // Refresh JWT token

/*
PUBLIC AUTH HANDLER ORGANIZATION:

TypeScript Pattern (monk-api):
├── src/public/auth/routes.ts           ← Barrel exports
├── src/public/auth/login/POST.ts       ← Login implementation
├── src/public/auth/register/POST.ts    ← Register implementation  
└── src/public/auth/refresh/POST.ts     ← Refresh implementation

Rust Pattern (monk-api-rust):
├── handlers/public/auth/mod.rs         ← This file (barrel exports)
├── handlers/public/auth/login.rs       ← Login implementation
├── handlers/public/auth/register.rs    ← Register implementation
└── handlers/public/auth/refresh.rs     ← Refresh implementation

AUTHENTICATION FLOW:

1. **Login**: POST /auth/login
   - Input: { "tenant": "string", "username": "string" }
   - Validates credentials against tenant database
   - Returns: JWT token + user information
   - Token enables access to /api/ protected endpoints

2. **Register**: POST /auth/register  
   - Input: { "tenant": "string", "username": "string" }
   - Creates new user account in tenant database
   - May be disabled in production environments
   - Returns: Success confirmation

3. **Refresh**: POST /auth/refresh
   - Input: { "token": "string" }
   - Validates existing JWT token (may be expired)
   - Returns: New JWT token with extended expiration
   - Enables seamless token renewal for long-running sessions

SECURITY CONSIDERATIONS:

- **Input Validation**: No trusted user context, validate everything
- **Rate Limiting**: Prevent brute force attacks on authentication
- **Audit Logging**: Log all authentication attempts for security
- **HTTPS Only**: Credentials must be encrypted in transit
- **Token Security**: JWT tokens should use strong signing algorithms

These handlers are critical for system security and should be implemented
with comprehensive error handling and security best practices.
*/