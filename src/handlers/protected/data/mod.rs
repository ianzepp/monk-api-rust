// handlers/data/mod.rs - Data handler module  
// Equivalent to monk-api/src/routes/data/routes.ts

use axum::{extract::Path, http::StatusCode, response::Json};
use serde_json::{json, Value};

// Handler modules - each corresponds to a route operation
pub mod schema_get;     // GET /api/data/:schema
pub mod schema_post;    // POST /api/data/:schema  
pub mod schema_put;     // PUT /api/data/:schema
pub mod schema_delete;  // DELETE /api/data/:schema
// TODO: Add record handlers later
// pub mod record_get;     // GET /api/data/:schema/:record
// pub mod record_put;     // PUT /api/data/:schema/:record  
// pub mod record_delete;  // DELETE /api/data/:schema/:record

// Re-export handler functions - mirrors your TypeScript barrel exports
pub use schema_get::schema_get;         // Equivalent to SchemaGet
pub use schema_post::schema_post;       // Equivalent to SchemaPost  
pub use schema_put::schema_put;         // Equivalent to SchemaPut
pub use schema_delete::schema_delete;   // Equivalent to SchemaDelete
// TODO: Re-export record handlers later
// pub use record_get::record_get;         // Equivalent to RecordGet
// pub use record_put::record_put;         // Equivalent to RecordPut
// pub use record_delete::record_delete;   // Equivalent to RecordDelete

/*
DATA HANDLER MAPPING:

Your TypeScript Structure:
├── routes/data/routes.ts                           ← Barrel exports
├── routes/data/:schema/GET.ts                     ← List records  
├── routes/data/:schema/POST.ts                    ← Create records
├── routes/data/:schema/PUT.ts                     ← Bulk update
├── routes/data/:schema/DELETE.ts                  ← Bulk delete
├── routes/data/:schema/:record/GET.ts             ← Get single record
├── routes/data/:schema/:record/PUT.ts             ← Update single record
└── routes/data/:schema/:record/DELETE.ts          ← Delete single record

Rust Structure:
├── handlers/data/mod.rs                           ← This file (barrel exports)
├── handlers/data/schema_get.rs                    ← List records
├── handlers/data/schema_post.rs                   ← Create records  
├── handlers/data/schema_put.rs                    ← Bulk update
├── handlers/data/schema_delete.rs                 ← Bulk delete
├── handlers/data/record_get.rs                    ← Get single record
├── handlers/data/record_put.rs                    ← Update single record
└── handlers/data/record_delete.rs                 ← Delete single record

Key Differences:
- Rust files use snake_case naming
- Path parameters extracted via Path<(String,)> or Path<(String, String)>
- Database operations are async with compile-time query checking via SQLx
- Error handling uses Result<T, E> instead of throwing exceptions
*/