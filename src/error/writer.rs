//! Errors from writing a blob

use crate::check::{DataSizeError, KeyError, KeyProblem, MAX_DEPTH};
use crate::types::DType;
use std::error::Error;
use std::{fmt, io};

/// Why writing a blob failed: a value the format can't hold (which the
/// parser would refuse to read back), or I/O failing
///
/// An I/O failure's cause is part of the message rather than
/// [`Error::source`], so printing the error once shows it.
#[non_exhaustive]
#[derive(Debug)]
pub enum WriteError {
    /// Writing to the destination failed
    Io(io::Error),
    /// Reading a file value's handle failed
    FileRead(io::Error),
    /// A file handle ended before the size it reported
    #[non_exhaustive]
    FileTooShort { size: u64, read: u64 },
    /// An object key contains `/`, which separates the parts of a path
    #[non_exhaustive]
    KeyContainsSlash { key: String },
    /// An object has the same key twice, at entries `first` and `second`
    #[non_exhaustive]
    DuplicateKey {
        key: String,
        first: usize,
        second: usize,
    },
    /// An array's data length disagrees with its element type and shape
    /// (`needed` is `None` when multiplying out the shape overflows a `u64`)
    #[non_exhaustive]
    SizeMismatch {
        dtype: DType,
        shape: Vec<u64>,
        needed: Option<u64>,
        actual: u64,
    },
    /// Values are nested more than [`MAX_DEPTH`] deep
    TooDeep,
    /// A value is longer than the length field the format stores it with
    #[non_exhaustive]
    TooLarge { field: LengthField, len: usize },
}

/// A length field of the format, and so a limit on what it measures
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LengthField {
    /// A string's length in bytes (u32)
    String,
    /// An object key's length in bytes (u16)
    Key,
    /// A file's mimetype length in bytes (u16)
    Mimetype,
    /// An object's entry count (u32)
    Object,
    /// A list's item count (u32)
    List,
    /// An array's dimension count (u8)
    ArrayDimensions,
}

impl fmt::Display for LengthField {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            LengthField::String => "string",
            LengthField::Key => "key",
            LengthField::Mimetype => "mimetype",
            LengthField::Object => "object",
            LengthField::List => "list",
            LengthField::ArrayDimensions => "array",
        })
    }
}

impl LengthField {
    /// What the field counts
    fn unit(self) -> &'static str {
        match self {
            LengthField::String | LengthField::Key | LengthField::Mimetype => "bytes",
            LengthField::Object => "entries",
            LengthField::List => "items",
            LengthField::ArrayDimensions => "dimensions",
        }
    }
}

impl fmt::Display for WriteError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            WriteError::Io(error) => write!(f, "I/O error while writing: {error}"),
            WriteError::FileRead(error) => write!(f, "reading a file handle failed: {error}"),
            WriteError::FileTooShort { size, read } => {
                write!(f, "file handle ended after {read} of its {size} bytes")
            }
            // The same messages the parser gives for the same problems
            WriteError::KeyContainsSlash { key } => KeyError {
                index: 0,
                key,
                problem: KeyProblem::ContainsSlash,
            }
            .fmt(f),
            WriteError::DuplicateKey { key, first, second } => KeyError {
                index: *second,
                key,
                problem: KeyProblem::Duplicate { first: *first },
            }
            .fmt(f),
            WriteError::SizeMismatch {
                dtype,
                shape,
                needed,
                actual,
            } => DataSizeError {
                dtype: *dtype,
                shape,
                needed: *needed,
                actual: *actual,
            }
            .fmt(f),
            WriteError::TooDeep => write!(f, "values nested more than {MAX_DEPTH} deep"),
            WriteError::TooLarge { field, len } => {
                let unit = field.unit();
                write!(
                    f,
                    "{field} has {len} {unit}, more than its length field holds"
                )
            }
        }
    }
}

impl Error for WriteError {}

impl From<io::Error> for WriteError {
    fn from(error: io::Error) -> Self {
        WriteError::Io(error)
    }
}

impl From<KeyError<'_>> for WriteError {
    fn from(error: KeyError<'_>) -> Self {
        let key = error.key.to_string();
        match error.problem {
            KeyProblem::ContainsSlash => WriteError::KeyContainsSlash { key },
            KeyProblem::Duplicate { first } => WriteError::DuplicateKey {
                key,
                first,
                second: error.index,
            },
        }
    }
}

impl From<DataSizeError<'_>> for WriteError {
    fn from(error: DataSizeError<'_>) -> Self {
        WriteError::SizeMismatch {
            dtype: error.dtype,
            shape: error.shape.to_vec(),
            needed: error.needed,
            actual: error.actual,
        }
    }
}
