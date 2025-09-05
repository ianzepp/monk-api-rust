/// Shared types used across the codebase

use serde::{Deserialize, Serialize};

/// Database operations supported throughout the system
/// Used by both the observer pipeline and individual records
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Operation {
    Create,
    Update,
    Delete,
    Select,
    Revert,  // Undo soft-delete by clearing trashed_at timestamp
}