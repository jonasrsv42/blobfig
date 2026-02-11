//! Value parsing

use super::array::parse_array_body;
use super::entry::parse_entry;
use super::file::parse_file_body;
use super::primitives::{f64_le, i64_le, u8_parser, u32_le};
use super::string::parse_string;
use crate::types::{ValueTag, ValueView};
use parsicomb::{ByteCursor, CodeLoc, Cursor, Parser, ParsicombError, ntimes};
use std::borrow::Cow;

/// Parse a value
pub fn parse_value<'a>()
-> impl Parser<'a, Cursor = ByteCursor<'a>, Output = ValueView<'a>, Error = ParsicombError<'a>> {
    ValueParser
}

struct ValueParser;

impl<'a> Parser<'a> for ValueParser {
    type Cursor = ByteCursor<'a>;
    type Output = ValueView<'a>;
    type Error = ParsicombError<'a>;

    fn parse(&self, cursor: Self::Cursor) -> Result<(Self::Output, Self::Cursor), Self::Error> {
        let (tag_byte, cursor) = u8_parser().parse(cursor)?;
        let tag = ValueTag::from_u8(tag_byte).ok_or_else(|| {
            let (data, pos) = cursor.inner();
            ParsicombError::SyntaxError {
                message: Cow::Owned(format!("Invalid value tag: 0x{:02X}", tag_byte)),
                loc: CodeLoc::new(data, pos.saturating_sub(1)),
            }
        })?;

        match tag {
            ValueTag::Bool => {
                let (b, cursor) = u8_parser().parse(cursor)?;
                Ok((ValueView::Bool(b != 0), cursor))
            }
            ValueTag::Int => {
                let (v, cursor) = i64_le().parse(cursor)?;
                Ok((ValueView::Int(v), cursor))
            }
            ValueTag::Float => {
                let (v, cursor) = f64_le().parse(cursor)?;
                Ok((ValueView::Float(v), cursor))
            }
            ValueTag::String => {
                let (s, cursor) = parse_string().parse(cursor)?;
                Ok((ValueView::String(s), cursor))
            }
            ValueTag::Array => {
                let (arr, cursor) = parse_array_body().parse(cursor)?;
                Ok((ValueView::Array(arr), cursor))
            }
            ValueTag::File => {
                let (file, cursor) = parse_file_body().parse(cursor)?;
                Ok((ValueView::File(file), cursor))
            }
            ValueTag::Object => {
                let (n, cursor) = u32_le().parse(cursor)?;
                let (entries, cursor) = ntimes(n as usize, parse_entry()).parse(cursor)?;
                Ok((ValueView::Object(entries), cursor))
            }
            ValueTag::List => {
                let (n, cursor) = u32_le().parse(cursor)?;
                let (items, cursor) = ntimes(n as usize, parse_value()).parse(cursor)?;
                Ok((ValueView::List(items), cursor))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
        bytes.extend_from_slice(&3.14f64.to_le_bytes());
        let cursor = ByteCursor::new(&bytes);
        let (val, _) = parse_value().parse(cursor).unwrap();
        let f = val.as_float().unwrap();
        assert!((f - 3.14).abs() < 1e-10);
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
        assert_eq!(obj[0].0, "name");
        assert_eq!(obj[0].1.as_str(), Some("test"));
        assert_eq!(obj[1].0, "count");
        assert_eq!(obj[1].1.as_int(), Some(42));
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
        let inner_val = val.get("inner.value").unwrap();
        assert_eq!(inner_val.as_int(), Some(123));
    }

    #[test]
    fn test_invalid_tag() {
        let bytes = &[0xFF]; // Invalid tag
        let cursor = ByteCursor::new(bytes);
        let result = parse_value().parse(cursor);
        assert!(result.is_err());
    }
}
