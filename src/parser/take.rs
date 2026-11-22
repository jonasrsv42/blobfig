//! Zero-copy byte runs: a fixed count, or a length read from the input

use parsicomb::combinator::{AndThenExt, MapErrExt};
use parsicomb::{ByteCursor, Cursor, MapExt, Parser, ParsicombError, parser_fn};

/// Read a length with `length`, then take that many bytes as a zero-copy slice
pub fn sized<'a, L: Into<u64>>(
    length: impl Parser<Cursor = ByteCursor<'a>, Output = L, Error = ParsicombError<'a>>,
) -> impl Parser<Cursor = ByteCursor<'a>, Output = &'a [u8], Error = ParsicombError<'a>> {
    length
        .map(|len| len.into())
        .and_then(take)
        .map_err(ParsicombError::from)
}

/// Take exactly `count` bytes as a zero-copy slice
///
/// `count` is a `u64` because lengths in the format are. A count too large for
/// `usize` (on a 32-bit target) can't fit in the input either, so like any
/// count past the end it fails as end of input, never truncated.
pub fn take<'a>(
    count: u64,
) -> impl Parser<Cursor = ByteCursor<'a>, Output = &'a [u8], Error = ParsicombError<'a>> {
    parser_fn(move |cursor: ByteCursor<'a>| {
        let (data, pos) = cursor.inner();
        let end = usize::try_from(count)
            .ok()
            .and_then(|count| pos.checked_add(count));
        match end {
            Some(end) if end <= data.len() => Ok((&data[pos..end], cursor.advance(end - pos))),
            _ => Err(ParsicombError::EndOfInput(cursor.loc())),
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::primitives::le;

    #[test]
    fn test_take_zero() {
        let data = b"hello";
        let cursor = ByteCursor::new(data);
        let (slice, cursor) = take(0).parse(cursor).unwrap();
        assert_eq!(slice, b"");
        assert_eq!(cursor.value().unwrap(), b'h');
    }

    #[test]
    fn test_take_some() {
        let data = b"hello world";
        let cursor = ByteCursor::new(data);
        let (slice, cursor) = take(5).parse(cursor).unwrap();
        assert_eq!(slice, b"hello");
        assert_eq!(cursor.value().unwrap(), b' ');
    }

    #[test]
    fn test_take_all() {
        let data = b"hello";
        let cursor = ByteCursor::new(data);
        let (slice, cursor) = take(5).parse(cursor).unwrap();
        assert_eq!(slice, b"hello");
        assert!(cursor.eos());
    }

    #[test]
    fn test_take_too_many() {
        let data = b"hi";
        let cursor = ByteCursor::new(data);
        let result = take(10).parse(cursor);
        assert!(result.is_err());
    }

    #[test]
    fn test_take_chained() {
        let data = b"helloworld";
        let cursor = ByteCursor::new(data);
        let (slice1, cursor) = take(5).parse(cursor).unwrap();
        let (slice2, _) = take(5).parse(cursor).unwrap();
        assert_eq!(slice1, b"hello");
        assert_eq!(slice2, b"world");
    }

    #[test]
    fn test_take_is_zero_copy() {
        let data = b"hello";
        let cursor = ByteCursor::new(data);
        let (slice, _) = take(5).parse(cursor).unwrap();

        // Verify slice points into original data
        assert!(std::ptr::eq(slice.as_ptr(), data.as_ptr()));
    }

    #[test]
    fn test_sized_too_short_is_end_of_input_after_length() {
        let mut data = 5u32.to_le_bytes().to_vec();
        data.extend_from_slice(b"ab");
        let error = sized(le(u32::from_le_bytes))
            .parse(ByteCursor::new(&data))
            .unwrap_err();
        assert!(matches!(error, ParsicombError::EndOfInput(_)));
        assert_eq!(error.position(), 4);
    }
}
