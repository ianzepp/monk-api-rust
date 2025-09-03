// handlers/meta/mod.rs - Meta handler module
// Equivalent to monk-api/src/routes/meta/routes.ts (but only schema operations)

use axum::{extract::Path, http::StatusCode, response::Json};
use serde_json::{json, Value};

// Handler modules for meta schema operations
pub mod schema_get;     // GET /api/meta/:schema
pub mod schema_post;    // POST /api/meta/:schema (create schema)
pub mod schema_put;     // PUT /api/meta/:schema (update schema)  
pub mod schema_delete;  // DELETE /api/meta/:schema

// Re-export handler functions
pub use schema_get::schema_get;         // Get schema definition
pub use schema_post::schema_post;       // Create new schema
pub use schema_put::schema_put;         // Update schema  
pub use schema_delete::schema_delete;   // Delete schema

/*
META HANDLER PURPOSE:

The meta endpoints manage JSON Schema definitions that generate PostgreSQL tables.
This is the "dynamic schema" feature that makes monk-api powerful.

Your TypeScript Flow:
1. POST /api/meta/:schema with YAML JSON Schema definition
2. Validates schema against JSON Schema spec  
3. Generates PostgreSQL CREATE TABLE statement
4. Creates database table automatically
5. Enables /api/data/:schema operations on new table

Rust Implementation Will:
1. Parse YAML to serde_json::Value  
2. Validate against JSON Schema specification
3. Generate PostgreSQL DDL using custom generator
4. Execute DDL via SQLx to create table
5. Cache schema definition for data operations

Key Files We'll Need:
- schema_validator.rs: JSON Schema validation logic
- ddl_generator.rs: PostgreSQL table generation  
- schema_registry.rs: In-memory schema caching
*/