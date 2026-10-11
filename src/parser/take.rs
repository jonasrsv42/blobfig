//! Zero-copy take combinator for binary parsing

use parsicomb::{ByteCursor, Cursor, Parser, ParsicombError, parser_fn};

/// Take exactly N bytes as a zero-copy slice
pub fn take<'a>(
    count: usize,
) -> impl Parser<Cursor = ByteCursor<'a>, Output = &'a [u8], Error = ParsicombError<'a>> {
    parser_fn(move |cursor: ByteCursor<'a>| {
        let (data, pos) = cursor.inner();
        match pos.checked_add(count) {
            Some(end) if end <= data.len() => Ok((&data[pos..end], cursor.advance(count))),
            _ => Err(ParsicombError::UnexpectedEndOfFile(cursor.loc())),
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use parsicomb::Cursor;

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
}
