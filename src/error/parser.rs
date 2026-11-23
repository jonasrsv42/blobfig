//! Errors from parsing a blob

use crate::types::VERSION;
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
pub enum ParseErrorReason {
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
    /// The header sets flags this library doesn't know, e.g. a blob from a
    /// newer writer
    #[non_exhaustive]
    UnsupportedFlags { found: u32 },
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
    /// The padding that aligns an array's or file's data isn't all zero, e.g.
    /// corrupt data
    BadPadding,
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

/// What a caller can do about a failure: the broad kind of a
/// [`ParseErrorReason`]
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReasonClass {
    /// The input isn't a blobfig blob at all
    NotBlobfig,
    /// A blobfig blob this library can't read, e.g. from a newer writer
    Unsupported,
    /// A blob that is cut off or damaged
    Corrupt,
    /// A blob that goes past a limit of this parser, such as nesting depth
    Limit,
}

impl ParseErrorReason {
    /// A stable snake_case name for the reason, e.g. `"bad_tag"`, for logs and
    /// other languages; an existing reason's code never changes
    pub fn code(self) -> &'static str {
        match self {
            ParseErrorReason::Truncated => "truncated",
            ParseErrorReason::BadMagic => "bad_magic",
            ParseErrorReason::UnsupportedVersion { .. } => "unsupported_version",
            ParseErrorReason::UnsupportedFlags { .. } => "unsupported_flags",
            ParseErrorReason::BadTag { .. } => "bad_tag",
            ParseErrorReason::BadDType { .. } => "bad_dtype",
            ParseErrorReason::BadBool { .. } => "bad_bool",
            ParseErrorReason::InvalidUtf8 => "invalid_utf8",
            ParseErrorReason::BadPadding => "bad_padding",
            ParseErrorReason::SizeMismatch { .. } => "size_mismatch",
            ParseErrorReason::KeyContainsSlash => "key_contains_slash",
            ParseErrorReason::DuplicateKey { .. } => "duplicate_key",
            ParseErrorReason::TooDeep => "too_deep",
            ParseErrorReason::TrailingBytes => "trailing_bytes",
        }
    }

    /// The broad kind of failure, for deciding what to do about it
    pub fn class(self) -> ReasonClass {
        match self {
            ParseErrorReason::BadMagic => ReasonClass::NotBlobfig,
            ParseErrorReason::UnsupportedVersion { .. }
            | ParseErrorReason::UnsupportedFlags { .. } => ReasonClass::Unsupported,
            ParseErrorReason::TooDeep => ReasonClass::Limit,
            ParseErrorReason::Truncated
            | ParseErrorReason::BadTag { .. }
            | ParseErrorReason::BadDType { .. }
            | ParseErrorReason::BadBool { .. }
            | ParseErrorReason::InvalidUtf8
            | ParseErrorReason::BadPadding
            | ParseErrorReason::SizeMismatch { .. }
            | ParseErrorReason::KeyContainsSlash
            | ParseErrorReason::DuplicateKey { .. }
            | ParseErrorReason::TrailingBytes => ReasonClass::Corrupt,
        }
    }
}

impl fmt::Display for ParseErrorReason {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match *self {
            ParseErrorReason::Truncated => write!(f, "unexpected end of input"),
            ParseErrorReason::BadMagic => write!(f, "not a blobfig file (bad magic bytes)"),
            ParseErrorReason::UnsupportedVersion { found } => {
                write!(f, "unsupported version {found}, expected {VERSION}")
            }
            ParseErrorReason::UnsupportedFlags { found } => {
                write!(f, "unsupported header flags: 0x{found:08X}")
            }
            ParseErrorReason::BadTag { found } => write!(f, "invalid value tag: 0x{found:02X}"),
            ParseErrorReason::BadDType { found } => write!(f, "invalid dtype: 0x{found:02X}"),
            ParseErrorReason::BadBool { found } => write!(f, "invalid bool: 0x{found:02X}"),
            ParseErrorReason::InvalidUtf8 => write!(f, "invalid UTF-8"),
            ParseErrorReason::BadPadding => {
                write!(f, "padding before array or file data isn't zero")
            }
            ParseErrorReason::SizeMismatch { needed, actual } => {
                write!(
                    f,
                    "array data is {actual} bytes, but its dtype and shape need "
                )?;
                match needed {
                    Some(bytes) => write!(f, "{bytes} bytes"),
                    None => write!(f, "more bytes than fit in a u64"),
                }
            }
            ParseErrorReason::KeyContainsSlash => write!(f, "key contains '/'"),
            ParseErrorReason::DuplicateKey { first } => {
                write!(f, "duplicate key (first at offset {first})")
            }
            ParseErrorReason::TooDeep => write!(f, "maximum nesting depth exceeded"),
            ParseErrorReason::TrailingBytes => write!(f, "bytes left over after the value"),
        }
    }
}

/// The error of [`parse`](crate::parse): why it failed, where, and a message
/// for people
///
/// Owns its data, so it outlives the input it was parsed from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParseError {
    reason: ParseErrorReason,
    offset: usize,
    message: String,
}

impl ParseError {
    pub(crate) fn new(reason: ParseErrorReason, offset: usize, message: String) -> Self {
        ParseError {
            reason,
            offset,
            message,
        }
    }

    /// Why parsing failed
    pub fn reason(&self) -> ParseErrorReason {
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    /// Codes and classes are read by other languages (blobfigy picks its
    /// exception class by class), so they must not change, and codes must not
    /// collide
    #[test]
    fn test_reason_codes_and_classes_are_stable() {
        let reasons = [
            (
                ParseErrorReason::Truncated,
                "truncated",
                ReasonClass::Corrupt,
            ),
            (
                ParseErrorReason::BadMagic,
                "bad_magic",
                ReasonClass::NotBlobfig,
            ),
            (
                ParseErrorReason::UnsupportedVersion { found: 0 },
                "unsupported_version",
                ReasonClass::Unsupported,
            ),
            (
                ParseErrorReason::UnsupportedFlags { found: 0 },
                "unsupported_flags",
                ReasonClass::Unsupported,
            ),
            (
                ParseErrorReason::BadTag { found: 0 },
                "bad_tag",
                ReasonClass::Corrupt,
            ),
            (
                ParseErrorReason::BadDType { found: 0 },
                "bad_dtype",
                ReasonClass::Corrupt,
            ),
            (
                ParseErrorReason::BadBool { found: 0 },
                "bad_bool",
                ReasonClass::Corrupt,
            ),
            (
                ParseErrorReason::InvalidUtf8,
                "invalid_utf8",
                ReasonClass::Corrupt,
            ),
            (
                ParseErrorReason::BadPadding,
                "bad_padding",
                ReasonClass::Corrupt,
            ),
            (
                ParseErrorReason::SizeMismatch {
                    needed: None,
                    actual: 0,
                },
                "size_mismatch",
                ReasonClass::Corrupt,
            ),
            (
                ParseErrorReason::KeyContainsSlash,
                "key_contains_slash",
                ReasonClass::Corrupt,
            ),
            (
                ParseErrorReason::DuplicateKey { first: 0 },
                "duplicate_key",
                ReasonClass::Corrupt,
            ),
            (ParseErrorReason::TooDeep, "too_deep", ReasonClass::Limit),
            (
                ParseErrorReason::TrailingBytes,
                "trailing_bytes",
                ReasonClass::Corrupt,
            ),
        ];
        for (reason, code, class) in reasons {
            assert_eq!((reason.code(), reason.class()), (code, class), "{reason:?}");
        }
        let codes: HashSet<_> = reasons.iter().map(|(reason, ..)| reason.code()).collect();
        assert_eq!(codes.len(), reasons.len());
    }
}
