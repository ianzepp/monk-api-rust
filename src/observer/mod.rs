// Observer system for processing database operations through a pipeline
// Based on superior Rust design from OBSERVER_SYSTEM.md

pub mod stateful_record;
pub mod context;
pub mod traits;
pub mod pipeline;
pub mod error;
pub mod implementations;

// Re-export core types
pub use context::*;
pub use traits::*;
pub use pipeline::*;
pub use error::*;
pub use stateful_record::*;
pub use implementations::*;
