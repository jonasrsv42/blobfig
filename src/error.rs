//! Error types for blobfig

use crate::types::ValueTag;
use std::error::Error;
use std::fmt;

/// Error for accessing values by path
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AccessError {
    /// Path not found in the blob
    NotFound { path: String },
    /// Value at path has wrong type
    TypeMismatch {
        path: String,
        expected: &'static str,
        actual: ValueTag,
    },
}

impl fmt::Display for AccessError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AccessError::NotFound { path } => write!(f, "path not found: {}", path),
            AccessError::TypeMismatch {
                path,
                expected,
                actual,
            } => write!(
                f,
                "type mismatch at '{}': expected {}, got {:?}",
                path, expected, actual
            ),
        }
    }
}

impl Error for AccessError {}
