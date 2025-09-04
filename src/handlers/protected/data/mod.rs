pub mod record;
pub mod schema;
pub mod utils;

// Re-export handler functions for use in routing
pub use record::get as record_get;
pub use record::put as record_put;
pub use record::patch as record_patch;
pub use record::delete as record_delete;
pub use record::restore as record_restore;

pub use schema::get as schema_get;
pub use schema::post as schema_post;
pub use schema::put as schema_put;
pub use schema::patch as schema_patch;
pub use schema::delete as schema_delete;