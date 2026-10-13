//! String parsing helpers

use super::primitives::{u16_le, u32_le};
use super::take::take;
use parsicomb::map::MapExt;
use parsicomb::{ByteCursor, Parser, ParsicombError, parser_fn};
use std::borrow::Cow;

/// Parse a u32 length-prefixed UTF-8 string (for values)
pub fn parse_string<'a>()
-> impl Parser<Cursor = ByteCursor<'a>, Output = &'a str, Error = ParsicombError<'a>> {
    length_prefixed_str(u32_le().map(|len| len as usize), "Invalid UTF-8 in string")
}

/// Parse a u16 length-prefixed UTF-8 string (for object keys)
pub fn parse_key<'a>()
-> impl Parser<Cursor = ByteCursor<'a>, Output = &'a str, Error = ParsicombError<'a>> {
    length_prefixed_str(u16_le().map(|len| len as usize), "Invalid UTF-8 in string")
}

/// Parse a length, then that many bytes as UTF-8; invalid UTF-8 is reported
/// with `invalid_utf8` at the start of the bytes
pub(super) fn length_prefixed_str<'a, P>(
    length: P,
    invalid_utf8: &'static str,
) -> impl Parser<Cursor = ByteCursor<'a>, Output = &'a str, Error = ParsicombError<'a>>
where
    P: Parser<Cursor = ByteCursor<'a>, Output = usize, Error = ParsicombError<'a>>,
{
    parser_fn(move |cursor: ByteCursor<'a>| {
        let (len, start) = length.parse(cursor)?;
        let (bytes, end) = take(len).parse(start)?;
        let s = std::str::from_utf8(bytes).map_err(|_| ParsicombError::Invalid {
            message: Cow::Borrowed(invalid_utf8),
            loc: start.loc(),
        })?;
        Ok((s, end))
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_string() {
        let mut bytes = Vec::new();
        bytes.extend_from_slice(&5u32.to_le_bytes());
        bytes.extend_from_slice(b"hello");

        let cursor = ByteCursor::new(&bytes);
        let (s, _) = parse_string().parse(cursor).unwrap();
        assert_eq!(s, "hello");
    }

    #[test]
    fn test_parse_key() {
        let mut bytes = Vec::new();
        bytes.extend_from_slice(&4u16.to_le_bytes());
        bytes.extend_from_slice(b"name");

        let cursor = ByteCursor::new(&bytes);
        let (s, _) = parse_key().parse(cursor).unwrap();
        assert_eq!(s, "name");
    }

    #[test]
    fn test_parse_empty_string() {
        let bytes = 0u32.to_le_bytes();
        let cursor = ByteCursor::new(&bytes);
        let (s, _) = parse_string().parse(cursor).unwrap();
        assert_eq!(s, "");
    }
}
