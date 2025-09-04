pub mod schema;

// Re-export handler functions for use in routing
pub use schema::get as schema_get;
pub use schema::post as schema_post;
pub use schema::put as schema_put;
pub use schema::delete as schema_delete;