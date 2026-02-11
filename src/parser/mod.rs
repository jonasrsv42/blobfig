//! Binary parser for blobfig format
//!
//! Zero-copy parsing that returns views into the original byte buffer.

mod array;
mod entry;
mod file;
mod primitives;
mod string;
mod take;
mod value;

pub use value::parse_value;

use crate::types::{HEADER_SIZE, MAGIC, VERSION, ValueView};
use parsicomb::{ByteCursor, CodeLoc, Parser, ParsicombError};
use std::borrow::Cow;

/// Parse a blobfig from bytes
///
/// Returns a ValueView that borrows from the input bytes.
/// For zero-copy parsing from memory-mapped files, the bytes
/// must remain valid for the lifetime of the returned ValueView.
pub fn parse(bytes: &[u8]) -> Result<ValueView<'_>, ParsicombError<'_>> {
    // Check minimum size for header
    if bytes.len() < HEADER_SIZE {
        return Err(ParsicombError::UnexpectedEndOfFile(CodeLoc::new(bytes, 0)));
    }

    // Validate magic bytes
    if &bytes[0..8] != MAGIC {
        return Err(ParsicombError::SyntaxError {
            message: Cow::Borrowed("Invalid magic bytes"),
            loc: CodeLoc::new(bytes, 0),
        });
    }

    // Validate version
    let version = u32::from_le_bytes([bytes[8], bytes[9], bytes[10], bytes[11]]);
    if version != VERSION {
        return Err(ParsicombError::SyntaxError {
            message: Cow::Owned(format!(
                "Unsupported version: {}, expected {}",
                version, VERSION
            )),
            loc: CodeLoc::new(bytes, 8),
        });
    }

    // Parse value starting after header
    let cursor = ByteCursor::new(&bytes[HEADER_SIZE..]);
    let (value, _) = parse_value().parse(cursor)?;

    Ok(value)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::Value;
    use crate::writer;

    #[test]
    fn test_roundtrip_bool() {
        let value = Value::Bool(true);
        let bytes = writer::to_bytes(value).unwrap();
        let parsed = parse(&bytes).unwrap();
        assert_eq!(parsed.as_bool(), Some(true));
    }

    #[test]
    fn test_roundtrip_int() {
        let value = Value::Int(-42);
        let bytes = writer::to_bytes(value).unwrap();
        let parsed = parse(&bytes).unwrap();
        assert_eq!(parsed.as_int(), Some(-42));
    }

    #[test]
    fn test_roundtrip_float() {
        let value = Value::Float(3.14159);
        let bytes = writer::to_bytes(value).unwrap();
        let parsed = parse(&bytes).unwrap();
        let f = parsed.as_float().unwrap();
        assert!((f - 3.14159).abs() < 1e-10);
    }

    #[test]
    fn test_roundtrip_string() {
        let value = Value::String("hello world".into());
        let bytes = writer::to_bytes(value).unwrap();
        let parsed = parse(&bytes).unwrap();
        assert_eq!(parsed.as_str(), Some("hello world"));
    }

    #[test]
    fn test_roundtrip_list() {
        let value = Value::List(vec![Value::Int(1), Value::Int(2), Value::Int(3)]);
        let bytes = writer::to_bytes(value).unwrap();
        let parsed = parse(&bytes).unwrap();
        let list = parsed.as_list().unwrap();
        assert_eq!(list.len(), 3);
        assert_eq!(list[0].as_int(), Some(1));
        assert_eq!(list[1].as_int(), Some(2));
        assert_eq!(list[2].as_int(), Some(3));
    }

    #[test]
    fn test_roundtrip_object() {
        let value = Value::Object(vec![
            ("name".into(), Value::String("test".into())),
            ("count".into(), Value::Int(42)),
        ]);
        let bytes = writer::to_bytes(value).unwrap();
        let parsed = parse(&bytes).unwrap();

        let obj = parsed.as_object().unwrap();
        assert_eq!(obj.len(), 2);
        assert_eq!(obj[0].0, "name");
        assert_eq!(obj[0].1.as_str(), Some("test"));
        assert_eq!(obj[1].0, "count");
        assert_eq!(obj[1].1.as_int(), Some(42));
    }

    #[test]
    fn test_roundtrip_nested() {
        let value = Value::Object(vec![
            (
                "config".into(),
                Value::Object(vec![
                    ("enabled".into(), Value::Bool(true)),
                    ("threshold".into(), Value::Float(0.5)),
                ]),
            ),
            (
                "items".into(),
                Value::List(vec![Value::String("a".into()), Value::String("b".into())]),
            ),
        ]);
        let bytes = writer::to_bytes(value).unwrap();
        let parsed = parse(&bytes).unwrap();

        // Test path accessor
        let enabled = parsed.get("config.enabled").unwrap();
        assert_eq!(enabled.as_bool(), Some(true));

        let threshold = parsed.get("config.threshold").unwrap();
        let f = threshold.as_float().unwrap();
        assert!((f - 0.5).abs() < 1e-10);
    }

    #[test]
    fn test_invalid_magic() {
        let bytes = vec![0x00; HEADER_SIZE]; // Wrong magic but right size
        let result = parse(&bytes);
        assert!(result.is_err());
    }

    #[test]
    fn test_invalid_version() {
        let mut bytes = Vec::new();
        bytes.extend_from_slice(MAGIC);
        bytes.extend_from_slice(&99u32.to_le_bytes()); // Wrong version
        bytes.extend_from_slice(&0u32.to_le_bytes()); // flags
        bytes.extend_from_slice(&0u64.to_le_bytes()); // padding to reach HEADER_SIZE
        let result = parse(&bytes);
        assert!(result.is_err());
    }
}
