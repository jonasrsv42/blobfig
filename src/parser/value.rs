//! Value parsing

use super::Failure;
use super::array::parse_array_body;
use super::file::parse_file_body;
use super::or_truncated::OrTruncatedExt;
use super::primitives::{checked_byte, le};
use super::rejection::Rejection;
use super::string::utf8;
use crate::check::{KeyError, KeyProblem, MAX_DEPTH, check_keys};
use crate::error::ParseErrorReason;
use crate::types::{ValueTag, ValueView};
use parsicomb::combinator::{
    AndExt, AndThenExt, MapErrExt, MapExt, MaxDepthError, MaxDepthExt, ReasonError,
};
use parsicomb::{ByteCursor, CodeLoc, Parser, ParsicombError, ntimes, parser_fn};

/// Parse a value, dispatching on its tag byte
pub fn parse_value<'a>()
-> impl Parser<Cursor = ByteCursor<'a>, Output = ValueView<'a>, Error = Failure<'a>> {
    parser_fn(|cursor: ByteCursor<'a>| -> Result<_, Failure<'a>> {
        let tag = checked_byte(ValueTag::from_u8, |found| ParseErrorReason::BadTag {
            found,
        });
        let (tag, cursor) = tag.parse(cursor)?;

        match tag {
            ValueTag::Bool => parse_bool().map(ValueView::Bool).parse(cursor),
            ValueTag::Int => le(i64::from_le_bytes).map(ValueView::Int).parse(cursor),
            ValueTag::Float => le(f64::from_le_bytes).map(ValueView::Float).parse(cursor),
            ValueTag::String => utf8(le(u32::from_le_bytes), "string")
                .map(ValueView::String)
                .parse(cursor),
            ValueTag::Array => parse_array_body().map(ValueView::Array).parse(cursor),
            ValueTag::File => parse_file_body().map(ValueView::File).parse(cursor),
            ValueTag::Object => parse_object_body(cursor),
            ValueTag::List => le(u32::from_le_bytes)
                .and_then(|n| ntimes(parse_value(), n as usize))
                .map(ValueView::list)
                .or_truncated()
                .parse(cursor),
            // Transparent wrapper: re-wrapped so it stays redacted when printed
            ValueTag::Secret => parse_value()
                .map(|inner| ValueView::Secret(Box::new(inner)))
                .parse(cursor),
        }
    })
    .max_depth(MAX_DEPTH)
    .map_err(|error| match error {
        // Only the limit itself is too deep; every failure inside keeps its own
        MaxDepthError::Inner(failure) => failure,
        MaxDepthError::TooDeep(limit) => Failure::from(ReasonError::new(
            ParseErrorReason::TooDeep,
            ParsicombError::from(limit),
        )),
    })
}

/// A bool's byte, which must be 0 or 1
fn parse_bool<'a>() -> impl Parser<Cursor = ByteCursor<'a>, Output = bool, Error = Failure<'a>> {
    let zero_or_one = |b| match b {
        0 => Some(false),
        1 => Some(true),
        _ => None,
    };
    checked_byte(zero_or_one, |found| ParseErrorReason::BadBool { found })
}

/// An object's entries, which are refused if their keys break the key rules
//
// Inlined into `parse_value` because it recurses through it: as a separate
// frame it costs about 0.9 KB more stack per nested object (release, x86-64)
#[inline(always)]
fn parse_object_body<'a>(
    cursor: ByteCursor<'a>,
) -> Result<(ValueView<'a>, ByteCursor<'a>), Failure<'a>> {
    let entry = || utf8(le(u16::from_le_bytes), "key").and(parse_value());
    let (entries, end) = le(u32::from_le_bytes)
        .and_then(|n| ntimes(entry(), n as usize))
        .or_truncated()
        .parse(cursor)?;
    let (input, _) = cursor.inner();
    check_keys(entries.iter().map(|(key, _)| *key))
        .map_err(|error| key_failure(input, &entries, error))?;
    Ok((ValueView::object(entries), end))
}

/// The failure for a key that `check_keys` refused, pointing at the key
fn key_failure<'a>(
    input: &'a [u8],
    entries: &[(&'a str, ValueView<'a>)],
    error: KeyError<'_>,
) -> Failure<'a> {
    // Keys are slices of the input, so their offsets are where they start in it
    let offset = |key: &str| key.as_ptr() as usize - input.as_ptr() as usize;
    let reason = match error.problem {
        KeyProblem::ContainsSlash => ParseErrorReason::KeyContainsSlash,
        KeyProblem::Duplicate { first } => ParseErrorReason::DuplicateKey {
            first: offset(entries[first].0),
        },
    };
    Rejection::new(reason, error.to_string()).at(CodeLoc::new(input, offset(error.key)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{NodeView, ObjectNode};

    #[test]
    fn test_parse_bool_true() {
        let bytes = &[ValueTag::Bool as u8, 1];
        let cursor = ByteCursor::new(bytes);
        let (val, _) = parse_value().parse(cursor).unwrap();
        assert_eq!(val.as_bool(), Some(true));
    }

    #[test]
    fn test_parse_bool_false() {
        let bytes = &[ValueTag::Bool as u8, 0];
        let cursor = ByteCursor::new(bytes);
        let (val, _) = parse_value().parse(cursor).unwrap();
        assert_eq!(val.as_bool(), Some(false));
    }

    #[test]
    fn test_parse_int() {
        let mut bytes = vec![ValueTag::Int as u8];
        bytes.extend_from_slice(&42i64.to_le_bytes());
        let cursor = ByteCursor::new(&bytes);
        let (val, _) = parse_value().parse(cursor).unwrap();
        assert_eq!(val.as_int(), Some(42));
    }

    #[test]
    fn test_parse_float() {
        let mut bytes = vec![ValueTag::Float as u8];
        bytes.extend_from_slice(&2.5f64.to_le_bytes());
        let cursor = ByteCursor::new(&bytes);
        let (val, _) = parse_value().parse(cursor).unwrap();
        let f = val.as_float().unwrap();
        assert!((f - 2.5).abs() < 1e-10);
    }

    #[test]
    fn test_parse_string() {
        let s = "hello";
        let mut bytes = vec![ValueTag::String as u8];
        bytes.extend_from_slice(&(s.len() as u32).to_le_bytes());
        bytes.extend_from_slice(s.as_bytes());
        let cursor = ByteCursor::new(&bytes);
        let (val, _) = parse_value().parse(cursor).unwrap();
        assert_eq!(val.as_str(), Some("hello"));
    }

    #[test]
    fn test_parse_list() {
        let mut bytes = vec![ValueTag::List as u8];
        bytes.extend_from_slice(&3u32.to_le_bytes()); // 3 items

        // Item 1: Int(1)
        bytes.push(ValueTag::Int as u8);
        bytes.extend_from_slice(&1i64.to_le_bytes());

        // Item 2: Int(2)
        bytes.push(ValueTag::Int as u8);
        bytes.extend_from_slice(&2i64.to_le_bytes());

        // Item 3: Int(3)
        bytes.push(ValueTag::Int as u8);
        bytes.extend_from_slice(&3i64.to_le_bytes());

        let cursor = ByteCursor::new(&bytes);
        let (val, _) = parse_value().parse(cursor).unwrap();

        let list = val.as_list().unwrap();
        assert_eq!(list.len(), 3);
        assert_eq!(list[0].as_int(), Some(1));
        assert_eq!(list[1].as_int(), Some(2));
        assert_eq!(list[2].as_int(), Some(3));
    }

    #[test]
    fn test_parse_object() {
        let mut bytes = vec![ValueTag::Object as u8];
        bytes.extend_from_slice(&2u32.to_le_bytes()); // 2 entries

        // Entry 1: "name" -> "test"
        let key1 = "name";
        bytes.extend_from_slice(&(key1.len() as u16).to_le_bytes());
        bytes.extend_from_slice(key1.as_bytes());
        bytes.push(ValueTag::String as u8);
        let val1 = "test";
        bytes.extend_from_slice(&(val1.len() as u32).to_le_bytes());
        bytes.extend_from_slice(val1.as_bytes());

        // Entry 2: "count" -> 42
        let key2 = "count";
        bytes.extend_from_slice(&(key2.len() as u16).to_le_bytes());
        bytes.extend_from_slice(key2.as_bytes());
        bytes.push(ValueTag::Int as u8);
        bytes.extend_from_slice(&42i64.to_le_bytes());

        let cursor = ByteCursor::new(&bytes);
        let (val, _) = parse_value().parse(cursor).unwrap();

        let obj = val.as_object().unwrap();
        assert_eq!(obj.len(), 2);
        let entries: Vec<_> = obj.entries().collect();
        assert_eq!(entries[0].0, "name");
        assert_eq!(entries[0].1.as_str(), Some("test"));
        assert_eq!(entries[1].0, "count");
        assert_eq!(entries[1].1.as_int(), Some(42));
    }

    #[test]
    fn test_parse_nested_object() {
        let mut bytes = vec![ValueTag::Object as u8];
        bytes.extend_from_slice(&1u32.to_le_bytes()); // 1 entry

        // Entry: "inner" -> { "value": 123 }
        let key = "inner";
        bytes.extend_from_slice(&(key.len() as u16).to_le_bytes());
        bytes.extend_from_slice(key.as_bytes());

        // Nested object
        bytes.push(ValueTag::Object as u8);
        bytes.extend_from_slice(&1u32.to_le_bytes());
        let inner_key = "value";
        bytes.extend_from_slice(&(inner_key.len() as u16).to_le_bytes());
        bytes.extend_from_slice(inner_key.as_bytes());
        bytes.push(ValueTag::Int as u8);
        bytes.extend_from_slice(&123i64.to_le_bytes());

        let cursor = ByteCursor::new(&bytes);
        let (val, _) = parse_value().parse(cursor).unwrap();

        // Use path accessor
        let inner_val = val.get("inner/value").unwrap();
        assert_eq!(inner_val.as_int(), Some(123));
    }
}
