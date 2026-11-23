//! Zero-copy byte runs: a fixed count, a length read from the input, or one
//! read from the input and aligned

use super::Failure;
use super::or_truncated::OrTruncatedExt;
use super::primitives::checked;
use crate::error::ParseErrorReason;
use crate::types::padding_at;
use parsicomb::combinator::{AndThenExt, TerminatedExt};
use parsicomb::{ByteCursor, Cursor, MapExt, Parser, ParsicombError, parser_fn};

/// Read a length with `length`, then take that many bytes as a zero-copy slice
pub fn sized<'a, L: Into<u64>>(
    length: impl Parser<Cursor = ByteCursor<'a>, Output = L, Error = Failure<'a>>,
) -> impl Parser<Cursor = ByteCursor<'a>, Output = &'a [u8], Error = Failure<'a>> {
    length.map(|len| len.into()).and_then(take).or_truncated()
}

/// Read a length with `length`, skip the zero padding up to the next multiple
/// of [`ALIGNMENT`](crate::ALIGNMENT) from the start of the input, then take
/// that many bytes as a zero-copy slice: how array and file data are stored
pub fn aligned<'a, L: Into<u64>>(
    length: impl Parser<Cursor = ByteCursor<'a>, Output = L, Error = Failure<'a>>,
) -> impl Parser<Cursor = ByteCursor<'a>, Output = &'a [u8], Error = Failure<'a>> {
    length
        .map(|len| len.into())
        .terminated_by(zero_padding())
        .and_then(take)
        .or_truncated()
}

/// The padding before aligned data, which must be all zero
fn zero_padding<'a>() -> impl Parser<Cursor = ByteCursor<'a>, Output = (), Error = Failure<'a>> {
    parser_fn(|cursor: ByteCursor<'a>| {
        let padding = padding_at(cursor.position() as u64);
        checked(take(padding), |bytes| {
            if bytes.iter().all(|&byte| byte == 0) {
                Ok(())
            } else {
                Err(ParseErrorReason::BadPadding.into())
            }
        })
        .parse(cursor)
    })
}

/// Take exactly `count` bytes as a zero-copy slice
///
/// `count` is a `u64` because lengths in the format are. A count too large for
/// `usize` (on a 32-bit target) can't fit in the input either, so like any
/// count past the end it fails as [`ParseErrorReason::Truncated`], instead of
/// being cut down to fit.
pub fn take<'a>(
    count: u64,
) -> impl Parser<Cursor = ByteCursor<'a>, Output = &'a [u8], Error = Failure<'a>> {
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
    .or_truncated()
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
