// handlers/mod.rs - 3-Tier Handler Architecture
// 
// This module implements the complete security model from monk-api TypeScript:
// Public (no auth) → Protected (JWT auth) → Elevated (root JWT auth)
//
// Declare the three security tiers
pub mod public;    // Tier 1: No authentication required (/auth/*)
pub mod protected; // Tier 2: JWT authentication required (/api/*)  
pub mod elevated;  // Tier 3: Root JWT authentication required (/api/root/*)

// Re-export all handlers organized by security tier
pub use public::*;    // Public authentication endpoints
pub use protected::*; // Standard API operations  
pub use elevated::*;  // Administrative operations

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