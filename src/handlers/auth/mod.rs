// handlers/auth/mod.rs - Auth handler module
// Equivalent to monk-api/src/routes/auth/routes.ts

// Import common Axum types that all auth handlers will use
use axum::{http::StatusCode, response::Json};
use serde_json::{json, Value};

// Declare handler modules - each file contains one route handler
pub mod whoami; // GET /api/auth/whoami  
pub mod sudo;   // POST /api/auth/sudo

// Re-export handler functions with descriptive names
// This mirrors your TypeScript: export { default as WhoamiGet }
pub use whoami::whoami_get;     // Equivalent to WhoamiGet
pub use sudo::sudo_post;        // Equivalent to SudoPost

/*
AUTH HANDLER ORGANIZATION:

TypeScript Pattern:
├── routes/auth/routes.ts        ← Barrel exports
├── routes/auth/whoami/GET.ts    ← Handler implementation  
└── routes/auth/sudo/POST.ts     ← Handler implementation

Rust Pattern:
├── handlers/auth/mod.rs         ← This file (barrel exports)
├── handlers/auth/whoami.rs      ← Handler implementation
└── handlers/auth/sudo.rs        ← Handler implementation

Key Differences:
- Rust uses modules (.rs files) instead of directories
- Rust function names are snake_case (whoami_get vs WhoamiGet)
- Rust modules are explicitly declared in mod.rs
- Types are imported at module level for all handlers to share
*/