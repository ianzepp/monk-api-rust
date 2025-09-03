// Import statements in Rust use the `use` keyword
// This brings external crate functionality into our namespace
use axum::{
    // HTTP status codes - similar to standard HTTP status codes
    http::StatusCode,
    // JSON response type - automatically serializes Rust data to JSON
    response::Json,
    // Routing macros - define HTTP methods and URL patterns
    routing::{get, post, put, delete},
    // Main router struct - combines all routes into a single application
    Router,
};
// Serde JSON types for dynamic JSON handling
use serde_json::{json, Value}; // `json!` macro creates JSON, `Value` is any JSON type
// Tower HTTP middleware for cross-cutting concerns
use tower_http::cors::CorsLayer;   // Cross-Origin Resource Sharing
use tower_http::trace::TraceLayer; // HTTP request/response logging

// Import our organized handler modules
// This is equivalent to your TypeScript: import * as authRoutes from '@src/routes/auth/routes.js'
mod handlers;                    // Declare the handlers module
use handlers::auth;             // Import auth handlers
use handlers::data;             // Import data handlers  
use handlers::meta;             // Import meta handlers

// The `#[tokio::main]` attribute macro transforms this into an async runtime
// Tokio is Rust's most popular async runtime - handles async/await execution
#[tokio::main]
async fn main() {
    // Initialize tracing/logging - this sets up structured logging for the app
    // `fmt()` creates a default formatter, `init()` starts the subscriber
    tracing_subscriber::fmt::init();

    // Build the application router - this defines all HTTP routes
    // `Router::new()` creates an empty router, then we chain methods to add routes
    let app = Router::new()
        // Basic server endpoints
        .route("/", get(root))
        .route("/health", get(health))
        
        // Auth API endpoints - imported from handlers::auth module  
        // Equivalent to your TypeScript: app.get('/api/auth/whoami', authRoutes.WhoamiGet)
        .route("/api/auth/whoami", get(auth::whoami_get))
        .route("/api/auth/sudo", post(auth::sudo_post))
        
        // Data API endpoints - imported from handlers::data module
        // Schema-level operations (no ID parameter)
        .route("/api/data/:schema", get(data::schema_get))      // List records
        .route("/api/data/:schema", post(data::schema_post))    // Create records  
        .route("/api/data/:schema", put(data::schema_put))      // Bulk update
        .route("/api/data/:schema", delete(data::schema_delete)) // Bulk delete
        
        // Record-level operations (with ID parameter) - TODO: implement these handlers
        // .route("/api/data/:schema/:record", get(data::record_get))
        // .route("/api/data/:schema/:record", put(data::record_put))  
        // .route("/api/data/:schema/:record", delete(data::record_delete))
        
        // Meta API endpoints - imported from handlers::meta module
        .route("/api/meta/:schema", get(meta::schema_get))      // Get schema definition
        .route("/api/meta/:schema", post(meta::schema_post))    // Create schema
        .route("/api/meta/:schema", put(meta::schema_put))      // Update schema
        .route("/api/meta/:schema", delete(meta::schema_delete)) // Delete schema
        // Add CORS middleware - allows cross-origin requests from browsers
        // `permissive()` allows all origins (development only!)
        .layer(CorsLayer::permissive())
        // Add tracing middleware - logs all HTTP requests/responses
        .layer(TraceLayer::new_for_http());

    // Create a TCP listener on address 0.0.0.0:3000
    // `await` pauses execution until the future completes
    // `unwrap()` panics if there's an error (not ideal for production)
    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await.unwrap();
    
    // Print startup message to console
    // `println!` is a macro (indicated by the !) for printing to stdout
    println!("🚀 Monk API Rust server listening on http://0.0.0.0:3000");
    
    // Start the HTTP server
    // `axum::serve()` takes the listener and app, returns a Future
    // The server runs indefinitely until the process is killed
    axum::serve(listener, app).await.unwrap();
}

// Async function that handles GET / requests
// `async fn` declares an asynchronous function
// Return type `Json<Value>` automatically serializes to JSON response
async fn root() -> Json<Value> {
    // `Json()` wraps the data and sets Content-Type: application/json
    // `json!` macro creates JSON from Rust literals - very convenient!
    Json(json!({
        "name": "monk-api-rust",
        "version": "0.1.0", 
        "status": "development",
        "description": "Rust rewrite of Monk API - PaaS management platform"
    }))
}

// Health check endpoint - returns current server status
async fn health() -> Json<Value> {
    Json(json!({
        "status": "ok",
        // `chrono::Utc::now()` gets current UTC timestamp
        // Serde automatically serializes DateTime to ISO 8601 string
        "timestamp": chrono::Utc::now()
    }))
}

// Handler functions are now organized in separate modules!
// 
// Instead of having all handlers in main.rs, we now use:
// - handlers::auth::whoami_get     (was auth_login_placeholder)
// - handlers::data::schema_get     (was data_list_placeholder)  
// - handlers::meta::schema_get     (was meta_schema_placeholder)
//
// This follows the same organizational pattern as your TypeScript monk-api:
// - TypeScript: src/routes/auth/routes.ts exports WhoamiGet, SudoPost
// - Rust:       src/handlers/auth/mod.rs re-exports whoami_get, sudo_post
//
// Benefits of this approach:
// 1. **Separation of Concerns**: Each handler has its own file
// 2. **Easy Testing**: Individual handler functions can be unit tested
// 3. **Code Organization**: Related handlers grouped by feature (auth, data, meta)
// 4. **Maintainability**: Large codebases stay organized as they grow
// 5. **Team Development**: Multiple developers can work on different modules
//
// This is the Rust equivalent of your TypeScript modular architecture!

/*
KEY RUST CONCEPTS DEMONSTRATED:

1. **Ownership & Borrowing**: Not heavily shown here, but Rust's memory safety 
   comes from ownership rules - each value has one owner, prevents data races

2. **Async/Await**: `async fn` creates functions that can pause execution,
   `await` pauses until a Future completes, Tokio manages the execution

3. **Macros**: `println!`, `json!`, `#[tokio::main]` - end with ! or wrapped in #[]
   Macros generate code at compile time, very powerful in Rust

4. **Error Handling**: `unwrap()` panics on error (bad for production),
   usually you'd use `?` operator or `match` for proper error handling

5. **Type System**: Return types like `Json<Value>` and `(StatusCode, Json<Value>)`
   are enforced at compile time - no runtime type errors!

6. **Traits**: `Json<Value>` implements the `IntoResponse` trait, so Axum
   knows how to convert it into an HTTP response automatically

7. **Method Chaining**: `.route().route().layer().layer()` - common Rust pattern
   for building complex objects step by step

8. **Pattern Matching**: Not shown here, but `match` is Rust's powerful
   control flow construct for handling different cases safely
*/