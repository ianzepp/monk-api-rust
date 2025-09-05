// Observer implementations organized by rings
// Each ring handles a specific phase of data processing

// Ring 0: Data Preparation - load existing data, merge updates
#[path = "0/data_preparation.rs"]
pub mod data_preparation;

// Ring 5: Database - SQL execution
#[path = "5/create_sql_executor.rs"]
pub mod create_sql_executor;
#[path = "5/delete_sql_executor.rs"]
pub mod delete_sql_executor;
#[path = "5/revert_sql_executor.rs"]
pub mod revert_sql_executor;
#[path = "5/select_sql_executor.rs"]
pub mod select_sql_executor;
#[path = "5/update_sql_executor.rs"]
pub mod update_sql_executor;

// Ring 6: Post-Database - DDL operations following record changes
#[path = "6/create_column_ddl.rs"]
pub mod create_column_ddl;
#[path = "6/create_schema_ddl.rs"]
pub mod create_schema_ddl;
#[path = "6/delete_column_ddl.rs"]
pub mod delete_column_ddl;
#[path = "6/delete_schema_ddl.rs"]
pub mod delete_schema_ddl;
#[path = "6/update_column_ddl.rs"]
pub mod update_column_ddl;
#[path = "6/update_schema_ddl.rs"]
pub mod update_schema_ddl;

// Helper for registering observers (not ring-specific)
pub mod sql_executors;
pub use sql_executors::*;

// Ring 0 re-exports
pub use data_preparation::*;

// Ring 5 re-exports
pub use create_sql_executor::*;
pub use delete_sql_executor::*;
pub use revert_sql_executor::*;
pub use select_sql_executor::*;
pub use update_sql_executor::*;

// Ring 6 re-exports
pub use create_column_ddl::*;
pub use create_schema_ddl::*;
pub use delete_column_ddl::*;
pub use delete_schema_ddl::*;
pub use update_column_ddl::*;
pub use update_schema_ddl::*;
