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
    use crate::types::{NodeView, Value, ValueTag};
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
        let enabled = parsed.get("config/enabled").unwrap();
        assert_eq!(enabled.as_bool(), Some(true));

        let threshold = parsed.get("config/threshold").unwrap();
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
        let result = parse(&bytes);
        assert!(result.is_err());
    }

    // =========================================================================
    // Secret values: redaction-on-print, transparent to programmatic access.
    // =========================================================================

    #[test]
    fn test_secret_roundtrips_and_redacts() {
        let value = Value::Object(vec![
            ("user".into(), Value::String("alice".into())),
            ("password".into(), Value::secret("hunter2")),
        ]);
        let bytes = writer::to_bytes(value).unwrap();
        let parsed = parse(&bytes).unwrap();

        // Printing the whole structure must not leak the secret.
        let dbg = format!("{:?}", parsed);
        assert!(!dbg.contains("hunter2"), "secret leaked in Debug: {dbg}");
        assert!(dbg.contains("<redacted>"), "no redaction marker: {dbg}");
        assert!(dbg.contains("alice"), "non-secret field missing: {dbg}");

        // Explicit, typed access sees through the secret.
        assert_eq!(parsed.get("password").unwrap().as_str(), Some("hunter2"));
        assert_eq!(parsed.string("password").unwrap(), "hunter2");
    }

    #[test]
    fn test_secret_object_subtree_redacted_but_navigable() {
        let value = Value::Object(vec![(
            "db".into(),
            Value::secret(Value::Object(vec![
                ("host".into(), Value::String("db.internal".into())),
                ("password".into(), Value::String("s3cr3t".into())),
            ])),
        )]);
        let bytes = writer::to_bytes(value).unwrap();
        let parsed = parse(&bytes).unwrap();

        // Whole subtree hidden, including nested fields, in pretty-print too.
        let dbg = format!("{:#?}", parsed);
        assert!(!dbg.contains("s3cr3t"), "{dbg}");
        assert!(!dbg.contains("db.internal"), "{dbg}");
        assert!(dbg.contains("<redacted>"), "{dbg}");

        // get() descends through the secret wrapper.
        assert_eq!(parsed.string("db/password").unwrap(), "s3cr3t");
        assert_eq!(parsed.string("db/host").unwrap(), "db.internal");

        // The secret node itself still redacts when printed directly.
        assert_eq!(format!("{:?}", parsed.get("db").unwrap()), "<redacted>");
    }

    #[test]
    fn test_secret_scalar_typed_access() {
        let value = Value::Object(vec![("port".into(), Value::secret(Value::Int(5432)))]);
        let bytes = writer::to_bytes(value).unwrap();
        let parsed = parse(&bytes).unwrap();

        assert_eq!(parsed.int("port").unwrap(), 5432);
        assert_eq!(parsed.get("port").unwrap().tag(), ValueTag::Secret);
        assert_eq!(format!("{:?}", parsed.get("port").unwrap()), "<redacted>");
    }

    #[test]
    fn test_secret_in_list_redacts_only_element() {
        let value = Value::List(vec![
            Value::String("public".into()),
            Value::secret("private"),
        ]);
        let bytes = writer::to_bytes(value).unwrap();
        let parsed = parse(&bytes).unwrap();

        let dbg = format!("{:?}", parsed);
        assert!(dbg.contains("public"), "{dbg}");
        assert!(!dbg.contains("private"), "secret leaked: {dbg}");
        assert!(dbg.contains("<redacted>"), "{dbg}");

        let list = parsed.as_list().unwrap();
        assert_eq!(list[1].as_str(), Some("private"));
    }

    #[test]
    fn test_secret_to_owned_preserves_redaction() {
        let value = Value::Object(vec![("k".into(), Value::secret("v"))]);
        let bytes = writer::to_bytes(value).unwrap();
        let parsed = parse(&bytes).unwrap();

        let owned = parsed.get("k").unwrap().to_owned();
        assert_eq!(format!("{:?}", owned), "<redacted>");
        assert_eq!(owned.tag(), ValueTag::Secret);
    }

    #[test]
    fn test_double_secret_peels_on_access() {
        let value = Value::Secret(Box::new(Value::secret("deep")));
        let bytes = writer::to_bytes(value).unwrap();
        let parsed = parse(&bytes).unwrap();

        assert_eq!(format!("{:?}", parsed), "<redacted>");
        // Typed access peels through both layers.
        assert_eq!(parsed.as_str(), Some("deep"));
    }

    #[test]
    fn test_secret_byte_identical_roundtrip() {
        // The secret marker travels on the wire, so re-serializing a parsed
        // secret produces the same bytes.
        let value = Value::Object(vec![("token".into(), Value::secret("abc123"))]);
        let bytes = writer::to_bytes(value).unwrap();
        let parsed = parse(&bytes).unwrap();
        let rebytes = writer::to_bytes(parsed.to_owned()).unwrap();
        assert_eq!(bytes, rebytes);
    }

    #[test]
    fn test_secret_binary_blob_redacts_and_roundtrips() {
        use crate::types::File;
        let value = Value::Object(vec![(
            "key".into(),
            Value::secret(Value::File(File::from_bytes(
                "application/octet-stream",
                vec![0xDE, 0xAD, 0xBE, 0xEF],
            ))),
        )]);
        let bytes = writer::to_bytes(value).unwrap();
        let parsed = parse(&bytes).unwrap();

        // No byte of the secret payload appears in the printed form.
        assert_eq!(format!("{:?}", parsed.get("key").unwrap()), "<redacted>");
        // Explicit access still reaches the bytes.
        assert_eq!(
            parsed.get("key").unwrap().as_file().unwrap().data,
            &[0xDE, 0xAD, 0xBE, 0xEF]
        );
    }

    #[test]
    fn test_typed_access_is_transparent_on_secret_object() {
        // Documented semantic: typed access sees *through* a secret, so
        // `as_object()` on a secret object yields its (plaintext) children.
        // Marking a parent secret protects bulk printing, not explicit drill-in.
        let value = Value::Object(vec![(
            "creds".into(),
            Value::secret(Value::Object(vec![(
                "u".into(),
                Value::String("admin".into()),
            )])),
        )]);
        let bytes = writer::to_bytes(value).unwrap();
        let parsed = parse(&bytes).unwrap();

        let creds = parsed.get("creds").unwrap();
        let entries = creds
            .as_object()
            .expect("typed access sees through the secret");
        assert_eq!(entries[0].0, "u");
        assert_eq!(entries[0].1.as_str(), Some("admin"));
    }

    #[test]
    fn test_type_mismatch_reports_inner_type_not_secret() {
        use crate::error::AccessError;
        let value = Value::Object(vec![("port".into(), Value::secret(Value::Int(5432)))]);
        let bytes = writer::to_bytes(value).unwrap();
        let parsed = parse(&bytes).unwrap();

        // Asking for the wrong type should name the real inner type, not "Secret".
        match parsed.string("port") {
            Err(AccessError::TypeMismatch { actual, .. }) => assert_eq!(actual, ValueTag::Int),
            other => panic!("expected TypeMismatch, got {other:?}"),
        }
    }
}
