//! Error types for blobfig

use crate::types::{VERSION, ValueTag};
use std::error::Error;
use std::fmt;

/// Why a blob failed to parse; [`ParseError::reason`] returns it
///
/// For code that needs to tell failures apart. Its `Display` is a short
/// description; the error's own message may say more. Variants that carry
/// data are `#[non_exhaustive]`, so match them with `{ .. }`: fields can be
/// added without breaking that.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Reason {
    /// The input ended before a value did, e.g. a cut-off file, or a corrupt
    /// length or count that claims more than the input holds. The offset is
    /// where the field that didn't fit starts.
    Truncated,
    /// The input doesn't start with the blobfig magic bytes, e.g. it isn't a
    /// blobfig file, or is shorter than the magic
    BadMagic,
    /// The header names a format version this library doesn't read
    #[non_exhaustive]
    UnsupportedVersion { found: u32 },
    /// A value starts with a byte that isn't a known value tag, e.g. corrupt
    /// data
    #[non_exhaustive]
    BadTag { found: u8 },
    /// An array names an element type that doesn't exist, e.g. corrupt data
    #[non_exhaustive]
    BadDType { found: u8 },
    /// A bool is stored as a byte other than 0 or 1
    #[non_exhaustive]
    BadBool { found: u8 },
    /// A string, key or mimetype isn't valid UTF-8
    InvalidUtf8,
    /// An array's data length disagrees with its element type and shape
    /// (`needed` is `None` when multiplying out the shape overflows a `u64`
    /// before reaching a zero dimension)
    #[non_exhaustive]
    SizeMismatch { needed: Option<u64>, actual: u64 },
    /// An object key contains `/`, which separates the parts of a path
    KeyContainsSlash,
    /// An object has the same key twice; the error's offset is the second,
    /// `first` the first
    #[non_exhaustive]
    DuplicateKey { first: usize },
    /// Values are nested deeper than the parser allows, e.g. hostile or
    /// corrupt input
    TooDeep,
    /// There are bytes left over after the blob's value
    TrailingBytes,
}

impl fmt::Display for Reason {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match *self {
            Reason::Truncated => write!(f, "unexpected end of input"),
            Reason::BadMagic => write!(f, "not a blobfig file (bad magic bytes)"),
            Reason::UnsupportedVersion { found } => {
                write!(f, "unsupported version {found}, expected {VERSION}")
            }
            Reason::BadTag { found } => write!(f, "invalid value tag: 0x{found:02X}"),
            Reason::BadDType { found } => write!(f, "invalid dtype: 0x{found:02X}"),
            Reason::BadBool { found } => write!(f, "invalid bool: 0x{found:02X}"),
            Reason::InvalidUtf8 => write!(f, "invalid UTF-8"),
            Reason::SizeMismatch { needed, actual } => {
                write!(
                    f,
                    "array data is {actual} bytes, but its dtype and shape need "
                )?;
                match needed {
                    Some(bytes) => write!(f, "{bytes} bytes"),
                    None => write!(f, "more bytes than fit in a u64"),
                }
            }
            Reason::KeyContainsSlash => write!(f, "key contains '/'"),
            Reason::DuplicateKey { first } => {
                write!(f, "duplicate key (first at offset {first})")
            }
            Reason::TooDeep => write!(f, "maximum nesting depth exceeded"),
            Reason::TrailingBytes => write!(f, "bytes left over after the value"),
        }
    }
}

/// The error of [`parse`](crate::parse): why it failed, where, and a message
/// for people
///
/// Owns its data, so it outlives the input it was parsed from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParseError {
    reason: Reason,
    offset: usize,
    message: String,
}

impl ParseError {
    pub(crate) fn new(reason: Reason, offset: usize, message: String) -> Self {
        ParseError {
            reason,
            offset,
            message,
        }
    }

    /// Why parsing failed
    pub fn reason(&self) -> Reason {
        self.reason
    }

    /// Where it failed: an offset into the parsed bytes, from 0 up to and
    /// including their length (an input that ends early fails at its end)
    pub fn offset(&self) -> usize {
        self.offset
    }

    /// What went wrong, for people
    pub fn message(&self) -> &str {
        &self.message
    }
}

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} at offset {}", self.message, self.offset)
    }
}

impl Error for ParseError {}

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
