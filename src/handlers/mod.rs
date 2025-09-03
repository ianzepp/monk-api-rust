// handlers/mod.rs - Main handler module declaration
// This is Rust's equivalent to TypeScript's index.ts barrel exports

// Declare sub-modules - tells Rust these modules exist
// Each corresponds to a route group like your TypeScript structure
pub mod auth;   // /api/auth/* routes
pub mod data;   // /api/data/* routes  
pub mod meta;   // /api/meta/* routes

// Re-export handler functions for easy importing
// This creates a "barrel export" pattern like your TypeScript routes.ts files
pub use auth::*;  // Re-export all auth handlers
pub use data::*;  // Re-export all data handlers
pub use meta::*;  // Re-export all meta handlers

/*
RUST MODULE SYSTEM EXPLANATION:

1. **mod.rs files**: Act like index.ts - they declare what's in a module
2. **pub mod**: Declares a public sub-module (like export in TS)
3. **pub use**: Re-exports functions from sub-modules (like export { default as } in TS)

Directory structure will be:
src/
├── handlers/
│   ├── mod.rs          ← This file (like routes/index.ts)
│   ├── auth/
│   │   ├── mod.rs      ← Auth module declaration
│   │   ├── whoami.rs   ← GET /api/auth/whoami handler
│   │   └── sudo.rs     ← POST /api/auth/sudo handler  
│   ├── data/
│   │   ├── mod.rs      ← Data module declaration
│   │   ├── schema_get.rs    ← GET /api/data/:schema handler
│   │   ├── schema_post.rs   ← POST /api/data/:schema handler
│   │   ├── record_get.rs    ← GET /api/data/:schema/:id handler
│   │   └── ...
│   └── meta/
│       ├── mod.rs      ← Meta module declaration
│       ├── schema_get.rs    ← GET /api/meta/:schema handler
│       └── ...
│
└── main.rs            ← Routes imported from handlers::*

This mirrors your TypeScript structure:
monk-api/src/routes/auth/routes.ts  →  handlers/auth/mod.rs
monk-api/src/routes/data/routes.ts  →  handlers/data/mod.rs
*/