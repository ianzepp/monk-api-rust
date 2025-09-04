use std::time::Duration;
use thiserror::Error;

/// Observer system errors with structured error types
#[derive(Debug, Error, Clone)]
pub enum ObserverError {
    #[error("Validation error: {0}")]
    ValidationError(String),
    
    #[error("Security error: {0}")]  
    SecurityError(String),
    
    #[error("System error: {0}")]
    SystemError(String),
    
    #[error("Timeout error: {0}")]
    TimeoutError(String),
    
    #[error("Database error: {0}")]
    DatabaseError(String),
    
    #[error("Observer recursion error: depth {depth} exceeds maximum {max_depth}")]
    RecursionError { depth: usize, max_depth: usize },
    
    #[error("Observer not found: {0}")]
    ObserverNotFound(String),
    
    #[error("Pipeline execution failed: {0}")]
    PipelineError(String),
}

/// Observer warnings (non-fatal issues)
#[derive(Debug, Clone)]
pub struct ObserverWarning {
    pub observer: String,
    pub ring: u8,
    pub message: String,
    pub context: Option<String>,
}

impl ObserverWarning {
    pub fn new(observer: &str, ring: u8, message: String) -> Self {
        Self {
            observer: observer.to_string(),
            ring,
            message,
            context: None,
        }
    }
    
    pub fn with_context(mut self, context: String) -> Self {
        self.context = Some(context);
        self
    }
}

/// Results from observer pipeline execution
#[derive(Debug, Clone)]
pub struct ObserverResult {
    pub success: bool,
    pub result: Option<Vec<serde_json::Value>>,
    pub errors: Vec<ObserverError>,
    pub warnings: Vec<ObserverWarning>,
    pub execution_time: Duration,
    pub rings_executed: Vec<crate::observer::traits::ObserverRing>,
}

impl ObserverResult {
    pub fn success(result: Vec<serde_json::Value>, execution_time: Duration, rings: Vec<crate::observer::traits::ObserverRing>) -> Self {
        Self {
            success: true,
            result: Some(result),
            errors: Vec::new(),
            warnings: Vec::new(),
            execution_time,
            rings_executed: rings,
        }
    }
    
    pub fn failure(errors: Vec<ObserverError>, execution_time: Duration) -> Self {
        Self {
            success: false,
            result: None,
            errors,
            warnings: Vec::new(),
            execution_time,
            rings_executed: Vec::new(),
        }
    }
}

/// Convert from database errors
impl From<crate::database::manager::DatabaseError> for ObserverError {
    fn from(error: crate::database::manager::DatabaseError) -> Self {
        ObserverError::DatabaseError(error.to_string())
    }
}