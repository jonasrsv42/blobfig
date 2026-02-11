//! String parsing helpers

use super::primitives::{u16_le, u32_le};
use super::take::take;
use parsicomb::{ByteCursor, CodeLoc, Cursor, Parser, ParsicombError};
use std::borrow::Cow;

/// Parse a u32 length-prefixed UTF-8 string (for values)
pub fn parse_string<'a>()
-> impl Parser<'a, Cursor = ByteCursor<'a>, Output = &'a str, Error = ParsicombError<'a>> {
    StringParser::<u32>::new()
}

/// Parse a u16 length-prefixed UTF-8 string (for object keys)
pub fn parse_key<'a>()
-> impl Parser<'a, Cursor = ByteCursor<'a>, Output = &'a str, Error = ParsicombError<'a>> {
    StringParser::<u16>::new()
}

struct StringParser<L> {
    _marker: std::marker::PhantomData<L>,
}

impl<L> StringParser<L> {
    fn new() -> Self {
        Self {
            _marker: std::marker::PhantomData,
        }
    }
}

impl<'a> Parser<'a> for StringParser<u32> {
    type Cursor = ByteCursor<'a>;
    type Output = &'a str;
    type Error = ParsicombError<'a>;

    fn parse(&self, cursor: Self::Cursor) -> Result<(Self::Output, Self::Cursor), Self::Error> {
        let (len, cursor) = u32_le().parse(cursor)?;
        let (bytes, cursor) = take(len as usize).parse(cursor)?;
        let s = std::str::from_utf8(bytes).map_err(|_| {
            let (data, pos) = cursor.inner();
            ParsicombError::SyntaxError {
                message: Cow::Borrowed("Invalid UTF-8 in string"),
                loc: CodeLoc::new(data, pos.saturating_sub(len as usize)),
            }
        })?;
        Ok((s, cursor))
    }
}

impl<'a> Parser<'a> for StringParser<u16> {
    type Cursor = ByteCursor<'a>;
    type Output = &'a str;
    type Error = ParsicombError<'a>;

    fn parse(&self, cursor: Self::Cursor) -> Result<(Self::Output, Self::Cursor), Self::Error> {
        let (len, cursor) = u16_le().parse(cursor)?;
        let (bytes, cursor) = take(len as usize).parse(cursor)?;
        let s = std::str::from_utf8(bytes).map_err(|_| {
            let (data, pos) = cursor.inner();
            ParsicombError::SyntaxError {
                message: Cow::Borrowed("Invalid UTF-8 in string"),
                loc: CodeLoc::new(data, pos.saturating_sub(len as usize)),
            }
        })?;
        Ok((s, cursor))
    }
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
