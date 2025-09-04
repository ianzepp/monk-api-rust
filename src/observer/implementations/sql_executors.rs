// Helper function for registering all SQL executors for REST API
use crate::observer::pipeline::ObserverPipeline;
use crate::observer::traits::ObserverBox;
use super::{
    CreateSqlExecutor, UpdateSqlExecutor, DeleteSqlExecutor, 
    RevertSqlExecutor, SelectSqlExecutor
};

/// Register all SQL executors for complete REST API CRUD support
/// Since this is a REST API, all CRUD operations must be available
pub fn register_all_sql_executors(pipeline: &mut ObserverPipeline) {
    pipeline.register_observer(ObserverBox::Database(Box::new(CreateSqlExecutor::default())));
    pipeline.register_observer(ObserverBox::Database(Box::new(UpdateSqlExecutor::default())));
    pipeline.register_observer(ObserverBox::Database(Box::new(DeleteSqlExecutor::default())));
    pipeline.register_observer(ObserverBox::Database(Box::new(RevertSqlExecutor::default())));
    pipeline.register_observer(ObserverBox::Database(Box::new(SelectSqlExecutor::default())));
}