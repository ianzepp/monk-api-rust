// handlers/data/schema_get.rs - GET /api/data/:schema handler
// Equivalent to monk-api/src/routes/data/:schema/GET.ts

use axum::{
    extract::Path,      // Extracts path parameters from URL
    http::StatusCode, 
    response::Json
};
use serde_json::{json, Value};

/**
 * GET /api/data/:schema - List all records in schema
 * 
 * Direct equivalent to your TypeScript handler:
 * ```typescript
 * export default withParams(async (context, { system, schema, options }) => {
 *     const result = await system.database.selectAny(schema!, {}, options);
 *     setRouteResult(context, result);
 * });
 * ```
 * 
 * @param schema - The schema name from URL path (like "users", "products")
 * @returns JSON array of records in the schema
 */
pub async fn schema_get(
    Path(schema): Path<String>,  // Extract :schema from /api/data/:schema
) -> (StatusCode, Json<Value>) {
    // TODO: Validate user has read access to this schema
    // TODO: Connect to database and query records
    // TODO: Apply pagination, filtering, sorting from query params
    // TODO: Return JSON array of records
    
    // Placeholder response showing the structure
    (
        StatusCode::NOT_IMPLEMENTED,
        Json(json!({
            "error": "Data schema GET not yet implemented",
            "message": format!("This will list all records in '{}' schema", schema),
            "schema": schema,
            "planned_response": [
                {
                    "id": "record_uuid_1",
                    "created_at": "2025-01-01T00:00:00Z",
                    "updated_at": "2025-01-01T00:00:00Z"
                    // ... other fields based on schema definition
                },
                {
                    "id": "record_uuid_2", 
                    "created_at": "2025-01-01T00:00:00Z",
                    "updated_at": "2025-01-01T00:00:00Z"
                }
            ]
        }))
    )
}

/*
RUST PATH EXTRACTION EXPLAINED:

1. **Path<String>**: Extracts a single path parameter
   - /api/data/:schema → Path<String> gets the :schema value
   
2. **Path<(String, String)>**: Extracts two path parameters  
   - /api/data/:schema/:record → Path<(String, String)> gets both values
   
3. **Path Parameter Types**:
   - String: Any text value
   - u32, i64: Numeric IDs 
   - Uuid: UUID validation at compile time!
   
4. **Query Parameters** (will add later):
   - Query<HashMap<String, String>>: All query params
   - Query<PaginationParams>: Custom struct for limit/offset
   
5. **Request Body** (for POST/PUT):
   - Json<CreateRecordRequest>: Deserialize JSON to Rust struct
   - Automatic validation based on struct field types!

EQUIVALENT FUNCTIONALITY:

TypeScript: context.params.schema
Rust:       Path(schema): Path<String>

TypeScript: system.database.selectAny(schema, {}, options) 
Rust:       sqlx::query!("SELECT * FROM {}", schema).fetch_all(&db).await?

The Rust version gives you:
- Compile-time validation that :schema parameter exists
- Type-safe database queries with SQLx macros
- Memory safety without runtime overhead
- Automatic JSON serialization/deserialization
*/