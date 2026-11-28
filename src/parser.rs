//! Binary parser for blobfig format
//!
//! Zero-copy parsing that returns views into the original byte buffer.

mod array;
mod file;
mod or_truncated;
mod primitives;
mod rejection;
mod string;
mod take;
mod value;

pub(crate) use value::parse_value;

use crate::error::{ParseError, ParseErrorReason};
use crate::types::{MAGIC, VERSION, ValueView};
use or_truncated::OrTruncatedExt;
use parsicomb::combinator::{AndExt, MapErrExt, PrecededExt, ReasonExt, TerminatedExt};
use parsicomb::{ByteCursor, ErrorNode, MapExt, Parser, ParsicombError, eof, is_sequence};
use primitives::{checked, le};

/// The error of the parsers inside: a parsicomb failure with a
/// [`ParseErrorReason`], turned into a [`ParseError`] once, by [`parse`]
pub(crate) type Failure<'a> = parsicomb::Failure<'a, ParseErrorReason>;

/// Parse a blobfig from bytes: the header, one value, and nothing after it
///
/// Returns a ValueView that borrows from the input bytes.
/// For zero-copy parsing from memory-mapped files, the bytes
/// must remain valid for the lifetime of the returned ValueView.
pub fn parse(bytes: &[u8]) -> Result<ValueView<'_>, ParseError> {
    let end = eof()
        .reason(ParseErrorReason::TrailingBytes)
        .map_err(Failure::from);
    let file = parse_value()
        .preceded_by(header())
        .terminated_by(end)
        .or_truncated();
    match file.parse(ByteCursor::new(bytes)) {
        Ok((value, _)) => Ok(value),
        Err(failure) => Err(to_parse_error(failure)),
    }
}

/// The owned error for a failure: its reason and offset, and the message the
/// parser gave, or else the reason's own description
///
/// The message is only recovered from a plain `ParsicombError::Invalid`,
/// which is how [`Rejection`](rejection::Rejection) builds every specific
/// one. A rule put under `commit`, `attempt` or `skip_then` would box its
/// errors, and their messages would fall back to the reason's description.
fn to_parse_error(failure: Failure<'_>) -> ParseError {
    let reason = failure.reason();
    let offset = failure.progress().position();
    let message = match ParsicombError::from(failure) {
        ParsicombError::Invalid { message, .. } => message.into_owned(),
        _ => reason.to_string(),
    };
    ParseError::new(reason, offset, message)
}

/// The 16-byte header: magic bytes, a u32 version that must be [`VERSION`],
/// and u32 flags, which must be 0 until a flag is defined
fn header<'a>() -> impl Parser<Cursor = ByteCursor<'a>, Output = (), Error = Failure<'a>> {
    let magic = is_sequence(&MAGIC[..])
        .reason(ParseErrorReason::BadMagic)
        .map_err(Failure::from);
    let version = checked(le(u32::from_le_bytes), |found| match found {
        VERSION => Ok(()),
        _ => Err(ParseErrorReason::UnsupportedVersion { found }.into()),
    });
    // A blob from a newer writer may set a flag that changes its layout, so
    // refusing it is safer than misreading it
    let flags = checked(le(u32::from_le_bytes), |found| match found {
        0 => Ok(()),
        _ => Err(ParseErrorReason::UnsupportedFlags { found }.into()),
    });
    magic.and(version).and(flags).map(|_| ()).or_truncated()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::check::MAX_DEPTH;
    use crate::types::{DType, NodeView, ObjectNode, Value, ValueTag};
    use crate::writer;

    /// Every way a blob can fail, as a caller sees it: the reason, the offset
    /// into the input, and the message
    #[test]
    fn test_parse_errors() {
        let header = |version: u32| {
            let mut bytes = MAGIC.to_vec();
            bytes.extend_from_slice(&version.to_le_bytes());
            bytes.extend_from_slice(&0u32.to_le_bytes()); // flags
            bytes
        };
        // The header, then `body`, which starts at offset 16
        let blob = |body: &[&[u8]]| [header(VERSION), body.concat()].concat();
        let (object, list, array, string) = (
            ValueTag::Object as u8,
            ValueTag::List as u8,
            ValueTag::Array as u8,
            ValueTag::String as u8,
        );
        let (bool, secret) = (ValueTag::Bool as u8, ValueTag::Secret as u8);
        let u16 = |n: u16| n.to_le_bytes();
        let u32 = |n: u32| n.to_le_bytes();
        let u64 = |n: u64| n.to_le_bytes();

        let cases: Vec<(&str, Vec<u8>, ParseErrorReason, usize, &str)> = vec![
            (
                "shorter than magic",
                b"BLOB".to_vec(),
                ParseErrorReason::BadMagic,
                4,
                "not a blobfig file (bad magic bytes)",
            ),
            (
                "wrong magic",
                [b"BLOBFIX\0".as_slice(), &header(VERSION)[8..], &[bool, 1]].concat(),
                ParseErrorReason::BadMagic,
                6,
                "not a blobfig file (bad magic bytes)",
            ),
            (
                "newer version",
                [header(99), vec![bool, 1]].concat(),
                ParseErrorReason::UnsupportedVersion { found: 99 },
                8,
                "unsupported version 99, expected 1",
            ),
            (
                "unknown flags",
                [&header(VERSION)[..12], &u32(1), &[bool, 1]].concat(),
                ParseErrorReason::UnsupportedFlags { found: 1 },
                12,
                "unsupported header flags: 0x00000001",
            ),
            (
                "no value",
                blob(&[]),
                ParseErrorReason::Truncated,
                16,
                "unexpected end of input",
            ),
            (
                "length past the end",
                blob(&[&[string], &u32(u32::MAX)]),
                ParseErrorReason::Truncated,
                21,
                "unexpected end of input",
            ),
            (
                "unknown tag",
                blob(&[&[0xFF]]),
                ParseErrorReason::BadTag { found: 0xFF },
                16,
                "invalid value tag: 0xFF",
            ),
            (
                "bool not 0 or 1",
                blob(&[&[bool, 2]]),
                ParseErrorReason::BadBool { found: 2 },
                17,
                "invalid bool: 0x02",
            ),
            (
                "unknown dtype",
                blob(&[&[array, 0xEE]]),
                ParseErrorReason::BadDType { found: 0xEE },
                17,
                "invalid dtype: 0xEE",
            ),
            (
                "array data too short",
                // The length field ends at 35: padding to 64, then the data
                blob(&[
                    &[array, DType::F32 as u8, 1],
                    &u64(1000),
                    &u64(3),
                    &[0; 29],
                    &[0; 3],
                ]),
                ParseErrorReason::SizeMismatch {
                    needed: Some(4000),
                    actual: 3,
                },
                17,
                "array data is 3 bytes, but F32 with shape [1000] needs 4000 bytes",
            ),
            (
                "array shape overflows",
                blob(&[
                    &[array, DType::U8 as u8, 2],
                    &u64(u64::MAX),
                    &u64(2),
                    &u64(0),
                    &[0; 21], // padding from 43 to 64
                ]),
                ParseErrorReason::SizeMismatch {
                    needed: None,
                    actual: 0,
                },
                17,
                "array data is 0 bytes, but U8 with shape [18446744073709551615, 2] needs more bytes than fit in a u64",
            ),
            (
                "padding not zero",
                blob(&[
                    &[array, DType::U8 as u8, 1],
                    &u64(1),
                    &u64(1),
                    &[1],
                    &[0; 28],
                    &[7],
                ]),
                ParseErrorReason::BadPadding,
                35,
                "padding before array or file data isn't zero",
            ),
            (
                "invalid UTF-8 in key",
                blob(&[&[object], &u32(1), &u16(2), b"a\xFF", &[bool, 1]]),
                ParseErrorReason::InvalidUtf8,
                24,
                "invalid UTF-8 in key",
            ),
            (
                "key with slash",
                blob(&[&[object], &u32(1), &u16(3), b"a/b", &[bool, 1]]),
                ParseErrorReason::KeyContainsSlash,
                23,
                r#"key contains '/': "a/b""#,
            ),
            (
                "duplicate key",
                blob(&[
                    &[object],
                    &u32(2),
                    &u16(1),
                    b"a",
                    &[bool, 1],
                    &u16(1),
                    b"a",
                    &[bool, 1],
                ]),
                ParseErrorReason::DuplicateKey { first: 23 },
                28,
                r#"duplicate key: "a" (entries 0 and 1)"#,
            ),
            (
                "nested too deep",
                blob(&[&[secret; MAX_DEPTH], &[bool, 1]]),
                ParseErrorReason::TooDeep,
                16 + MAX_DEPTH,
                "maximum nesting depth exceeded",
            ),
            (
                "too deep inside a list",
                blob(&[&[list], &u32(1), &[secret; MAX_DEPTH - 1], &[bool, 1]]),
                ParseErrorReason::TooDeep,
                21 + MAX_DEPTH - 1,
                "maximum nesting depth exceeded",
            ),
            (
                "trailing bytes",
                blob(&[&[bool, 1, 0]]),
                ParseErrorReason::TrailingBytes,
                18,
                "bytes left over after the value",
            ),
        ];

        for (name, input, reason, offset, message) in cases {
            let error = parse(&input).unwrap_err();
            assert_eq!(
                (error.reason(), error.offset(), error.message()),
                (reason, offset, message),
                "{name}"
            );
        }

        let error = parse(&blob(&[&[0xFF]])).unwrap_err();
        assert_eq!(error.to_string(), "invalid value tag: 0xFF at offset 16");
    }

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
        let value = Value::Float(2.5);
        let bytes = writer::to_bytes(value).unwrap();
        let parsed = parse(&bytes).unwrap();
        let f = parsed.as_float().unwrap();
        assert!((f - 2.5).abs() < 1e-10);
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
        let value = Value::list(vec![Value::Int(1), Value::Int(2), Value::Int(3)]);
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
        let value = Value::object(vec![
            ("name".into(), Value::String("test".into())),
            ("count".into(), Value::Int(42)),
        ]);
        let bytes = writer::to_bytes(value).unwrap();
        let parsed = parse(&bytes).unwrap();

        let obj = parsed.as_object().unwrap();
        assert_eq!(obj.len(), 2);
        let entries: Vec<_> = obj.entries().collect();
        assert_eq!(entries[0].0, "name");
        assert_eq!(entries[0].1.as_str(), Some("test"));
        assert_eq!(entries[1].0, "count");
        assert_eq!(entries[1].1.as_int(), Some(42));
    }

    #[test]
    fn test_roundtrip_nested() {
        let value = Value::object(vec![
            (
                "config".into(),
                Value::object(vec![
                    ("enabled".into(), Value::Bool(true)),
                    ("threshold".into(), Value::Float(0.5)),
                ]),
            ),
            (
                "items".into(),
                Value::list(vec![Value::String("a".into()), Value::String("b".into())]),
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

    // =========================================================================
    // Secret values: redaction-on-print, transparent to programmatic access.
    // =========================================================================

    #[test]
    fn test_secret_roundtrips_and_redacts() {
        let value = Value::object(vec![
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
        let value = Value::object(vec![(
            "db".into(),
            Value::secret(Value::object(vec![
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
        let value = Value::object(vec![("port".into(), Value::secret(Value::Int(5432)))]);
        let bytes = writer::to_bytes(value).unwrap();
        let parsed = parse(&bytes).unwrap();

        assert_eq!(parsed.int("port").unwrap(), 5432);
        assert_eq!(parsed.get("port").unwrap().tag(), ValueTag::Secret);
        assert_eq!(format!("{:?}", parsed.get("port").unwrap()), "<redacted>");
    }

    #[test]
    fn test_secret_in_list_redacts_only_element() {
        let value = Value::list(vec![
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
        let value = Value::object(vec![("k".into(), Value::secret("v"))]);
        let bytes = writer::to_bytes(value).unwrap();
        let parsed = parse(&bytes).unwrap();

        let owned = parsed.get("k").unwrap().to_owned();
        assert_eq!(format!("{:?}", owned), "<redacted>");
        assert_eq!(owned.tag(), ValueTag::Secret);
    }

    #[test]
    fn test_double_secret_peels_on_access() {
        let value = Value::secret(Value::secret("deep"));
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
        let value = Value::object(vec![("token".into(), Value::secret("abc123"))]);
        let bytes = writer::to_bytes(value).unwrap();
        let parsed = parse(&bytes).unwrap();
        let rebytes = writer::to_bytes(parsed.to_owned()).unwrap();
        assert_eq!(bytes, rebytes);
    }

    #[test]
    fn test_secret_binary_blob_redacts_and_roundtrips() {
        use crate::types::File;
        let value = Value::object(vec![(
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
        let value = Value::object(vec![(
            "creds".into(),
            Value::secret(Value::object(vec![(
                "u".into(),
                Value::String("admin".into()),
            )])),
        )]);
        let bytes = writer::to_bytes(value).unwrap();
        let parsed = parse(&bytes).unwrap();

        let creds = parsed.get("creds").unwrap();
        let object = creds
            .as_object()
            .expect("typed access sees through the secret");
        let entries: Vec<_> = object.entries().collect();
        assert_eq!(entries[0].0, "u");
        assert_eq!(entries[0].1.as_str(), Some("admin"));
    }

    #[test]
    fn test_type_mismatch_reports_inner_type_not_secret() {
        use crate::error::AccessError;
        let value = Value::object(vec![("port".into(), Value::secret(Value::Int(5432)))]);
        let bytes = writer::to_bytes(value).unwrap();
        let parsed = parse(&bytes).unwrap();

        // Asking for the wrong type should name the real inner type, not "Secret".
        match parsed.string("port") {
            Err(AccessError::TypeMismatch { actual, .. }) => assert_eq!(actual, ValueTag::Int),
            other => panic!("expected TypeMismatch, got {other:?}"),
        }
    }
}
