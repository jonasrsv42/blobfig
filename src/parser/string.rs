//! Length-prefixed UTF-8 strings

use super::Failure;
use super::rejection::Rejection;
use super::take::sized;
use crate::error::ParseErrorReason;
use parsicomb::{ByteCursor, CodeLoc, Parser, parser_fn};

/// Read a length with `length`, then that many bytes as UTF-8 (zero-copy)
///
/// Invalid UTF-8 is reported as "invalid UTF-8 in `what`" at the first byte
/// that isn't valid.
pub fn utf8<'a, L: Into<u64>>(
    length: impl Parser<Cursor = ByteCursor<'a>, Output = L, Error = Failure<'a>>,
    what: &'static str,
) -> impl Parser<Cursor = ByteCursor<'a>, Output = &'a str, Error = Failure<'a>> {
    let bytes = sized(length);
    parser_fn(move |cursor: ByteCursor<'a>| {
        let (bytes, end) = bytes.parse(cursor)?;
        let text = std::str::from_utf8(bytes).map_err(|error| {
            // At the first byte that isn't valid
            let (data, end_position) = end.inner();
            let at = end_position - bytes.len() + error.valid_up_to();
            let message = format!("invalid UTF-8 in {what}");
            Rejection::new(ParseErrorReason::InvalidUtf8, message).at(CodeLoc::new(data, at))
        })?;
        Ok((text, end))
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::primitives::le;

    fn string<'a>() -> impl Parser<Cursor = ByteCursor<'a>, Output = &'a str, Error = Failure<'a>> {
        utf8(le(u32::from_le_bytes), "string")
    }

    #[test]
    fn test_parse_string() {
        let mut bytes = Vec::new();
        bytes.extend_from_slice(&5u32.to_le_bytes());
        bytes.extend_from_slice(b"hello");

        let (s, _) = string().parse(ByteCursor::new(&bytes)).unwrap();
        assert_eq!(s, "hello");
    }

    #[test]
    fn test_parse_u16_length() {
        let mut bytes = Vec::new();
        bytes.extend_from_slice(&4u16.to_le_bytes());
        bytes.extend_from_slice(b"name");

        let (s, _) = utf8(le(u16::from_le_bytes), "key")
            .parse(ByteCursor::new(&bytes))
            .unwrap();
        assert_eq!(s, "name");
    }

    #[test]
    fn test_parse_empty_string() {
        let bytes = 0u32.to_le_bytes();
        let (s, _) = string().parse(ByteCursor::new(&bytes)).unwrap();
        assert_eq!(s, "");
    }
}
