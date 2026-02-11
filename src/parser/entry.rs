//! Object entry parsing

use super::string::parse_key;
use super::value::parse_value;
use crate::types::ValueView;
use parsicomb::{ByteCursor, Parser, ParsicombError};

/// Parse a single object entry (key + value)
pub fn parse_entry<'a>() -> impl Parser<
    'a,
    Cursor = ByteCursor<'a>,
    Output = (&'a str, ValueView<'a>),
    Error = ParsicombError<'a>,
> {
    EntryParser
}

struct EntryParser;

impl<'a> Parser<'a> for EntryParser {
    type Cursor = ByteCursor<'a>;
    type Output = (&'a str, ValueView<'a>);
    type Error = ParsicombError<'a>;

    fn parse(&self, cursor: Self::Cursor) -> Result<(Self::Output, Self::Cursor), Self::Error> {
        let (key, cursor) = parse_key().parse(cursor)?;
        let (value, cursor) = parse_value().parse(cursor)?;
        Ok(((key, value), cursor))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::ValueTag;

    #[test]
    fn test_parse_entry() {
        let mut bytes = Vec::new();
        // Key: "count"
        bytes.extend_from_slice(&5u16.to_le_bytes());
        bytes.extend_from_slice(b"count");
        // Value: Int(42)
        bytes.push(ValueTag::Int as u8);
        bytes.extend_from_slice(&42i64.to_le_bytes());

        let cursor = ByteCursor::new(&bytes);
        let ((key, value), _) = parse_entry().parse(cursor).unwrap();
        assert_eq!(key, "count");
        assert_eq!(value.as_int(), Some(42));
    }
}
