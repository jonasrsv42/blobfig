//! Error types for blobfig

use crate::types::ValueTag;
use parsicomb::{CodeLoc, ErrorLeaf, ErrorNode, ParsicombError};
use std::borrow::Cow;
use std::error::Error;
use std::fmt;

/// Blobfig-specific error type
#[derive(Debug)]
pub enum BlobfigError<'a> {
    /// Invalid magic bytes
    InvalidMagic,
    /// Unsupported version
    UnsupportedVersion(u32),
    /// Invalid value tag
    InvalidValueTag(u8),
    /// Invalid dtype tag
    InvalidDType(u8),
    /// Invalid UTF-8 in string
    InvalidUtf8,
    /// Data size mismatch
    DataSizeMismatch { expected: u64, actual: u64 },
    /// Wrapped parsicomb error
    Parse(ParsicombError<'a>),
    /// Generic error with message and location
    Custom {
        message: Cow<'static, str>,
        loc: CodeLoc<'a>,
    },
}

impl<'a> fmt::Display for BlobfigError<'a> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            BlobfigError::InvalidMagic => write!(f, "Invalid magic bytes, not a blobfig file"),
            BlobfigError::UnsupportedVersion(v) => write!(f, "Unsupported blobfig version: {}", v),
            BlobfigError::InvalidValueTag(tag) => write!(f, "Invalid value tag: 0x{:02X}", tag),
            BlobfigError::InvalidDType(tag) => write!(f, "Invalid dtype tag: 0x{:02X}", tag),
            BlobfigError::InvalidUtf8 => write!(f, "Invalid UTF-8 in string"),
            BlobfigError::DataSizeMismatch { expected, actual } => {
                write!(
                    f,
                    "Data size mismatch: expected {} bytes, got {}",
                    expected, actual
                )
            }
            BlobfigError::Parse(e) => write!(f, "{}", e),
            BlobfigError::Custom { message, loc } => {
                write!(f, "{} at position {}", message, loc.position())
            }
        }
    }
}

impl<'a> Error for BlobfigError<'a> {}

impl<'a> From<ParsicombError<'a>> for BlobfigError<'a> {
    fn from(e: ParsicombError<'a>) -> Self {
        BlobfigError::Parse(e)
    }
}

impl<'a> ErrorLeaf<'a> for BlobfigError<'a> {
    type Element = u8;

    fn loc(&self) -> CodeLoc<'a, Self::Element> {
        match self {
            BlobfigError::Parse(e) => e.loc(),
            BlobfigError::Custom { loc, .. } => *loc,
            // For errors without location info, return position 0
            _ => CodeLoc::new(&[], 0),
        }
    }
}

impl<'a> ErrorNode<'a> for BlobfigError<'a> {
    type Element = u8;

    fn likely_error(&self) -> &dyn ErrorLeaf<'a, Element = Self::Element> {
        self
    }
}

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
