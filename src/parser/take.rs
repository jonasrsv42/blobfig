//! Zero-copy take combinator for binary parsing

use parsicomb::{ByteCursor, CodeLoc, Cursor, Parser, ParsicombError};

/// Parser that takes exactly N bytes as a zero-copy slice
pub struct Take {
    count: usize,
}

impl Take {
    pub fn new(count: usize) -> Self {
        Take { count }
    }
}

impl<'a> Parser<'a> for Take {
    type Cursor = ByteCursor<'a>;
    type Output = &'a [u8];
    type Error = ParsicombError<'a>;

    fn parse(&self, cursor: Self::Cursor) -> Result<(Self::Output, Self::Cursor), Self::Error> {
        let (data, pos) = cursor.inner();

        if pos + self.count > data.len() {
            return Err(ParsicombError::UnexpectedEndOfFile(CodeLoc::new(data, pos)));
        }

        let slice = &data[pos..pos + self.count];
        let new_pos = pos + self.count;

        let new_cursor = if new_pos >= data.len() {
            ByteCursor::EndOfFile { data }
        } else {
            ByteCursor::Valid {
                data,
                position: new_pos,
            }
        };

        Ok((slice, new_cursor))
    }
}

/// Take exactly N bytes as a zero-copy slice
pub fn take(count: usize) -> Take {
    Take::new(count)
}

#[cfg(test)]
mod tests {
    use super::*;

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
