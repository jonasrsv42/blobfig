//! Fixed-width fields and checks: little-endian numbers, checked values

use super::Failure;
use super::or_truncated::OrTruncatedExt;
use super::rejection::Rejection;
use crate::error::Reason;
use parsicomb::{ByteCursor, Cursor, Parser, ParsicombError, parser_fn};

/// Run `parser`, then `check` its output; a rejected output is reported at
/// the start of what `parser` read
pub fn checked<'a, T, U>(
    parser: impl Parser<Cursor = ByteCursor<'a>, Output = T, Error = Failure<'a>>,
    check: impl Fn(T) -> Result<U, Rejection>,
) -> impl Parser<Cursor = ByteCursor<'a>, Output = U, Error = Failure<'a>> {
    parser_fn(move |cursor: ByteCursor<'a>| {
        let (value, next) = parser.parse(cursor)?;
        match check(value) {
            Ok(value) => Ok((value, next)),
            Err(rejection) => Err(rejection.at(cursor.loc())),
        }
    })
}

/// Read one byte and convert it with `convert`; a byte it refuses (`None`)
/// fails at that byte with `reason`, e.g.
/// `checked_byte(DType::from_u8, |found| Reason::BadDType { found })`
pub fn checked_byte<'a, T>(
    convert: fn(u8) -> Option<T>,
    reason: fn(u8) -> Reason,
) -> impl Parser<Cursor = ByteCursor<'a>, Output = T, Error = Failure<'a>> {
    checked(le(u8::from_le_bytes), move |b| {
        convert(b).ok_or_else(|| reason(b).into())
    })
}

/// Parse `N` bytes as a little-endian number, converted with `from`, e.g.
/// `le(u32::from_le_bytes)`
pub fn le<'a, const N: usize, T>(
    from: fn([u8; N]) -> T,
) -> impl Parser<Cursor = ByteCursor<'a>, Output = T, Error = Failure<'a>> {
    parser_fn(move |cursor: ByteCursor<'a>| {
        let (data, pos) = cursor.inner();
        match data.get(pos..).and_then(<[u8]>::first_chunk::<N>) {
            Some(chunk) => Ok((from(*chunk), cursor.advance(N))),
            None => Err(ParsicombError::EndOfInput(cursor.loc())),
        }
    })
    .or_truncated()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_u16_le() {
        let data = &[0x01, 0x02]; // 0x0201 in LE
        let (val, _) = le(u16::from_le_bytes).parse(ByteCursor::new(data)).unwrap();
        assert_eq!(val, 0x0201);
    }

    #[test]
    fn test_u32_le() {
        let data = &[0x01, 0x02, 0x03, 0x04];
        let (val, _) = le(u32::from_le_bytes).parse(ByteCursor::new(data)).unwrap();
        assert_eq!(val, 0x04030201);
    }

    #[test]
    fn test_u64_le() {
        let data = &[0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08];
        let (val, _) = le(u64::from_le_bytes).parse(ByteCursor::new(data)).unwrap();
        assert_eq!(val, 0x0807060504030201);
    }

    #[test]
    fn test_i64_le() {
        let data = (-42i64).to_le_bytes();
        let (val, _) = le(i64::from_le_bytes)
            .parse(ByteCursor::new(&data))
            .unwrap();
        assert_eq!(val, -42);
    }

    #[test]
    fn test_f64_le() {
        let data = (2.5f64).to_le_bytes();
        let (val, _) = le(f64::from_le_bytes)
            .parse(ByteCursor::new(&data))
            .unwrap();
        assert_eq!(val, 2.5);
    }

    #[test]
    fn test_chained() {
        let mut data = Vec::new();
        data.extend_from_slice(&42u32.to_le_bytes());
        data.extend_from_slice(&123i64.to_le_bytes());

        let cursor = ByteCursor::new(&data);
        let (v1, cursor) = le(u32::from_le_bytes).parse(cursor).unwrap();
        let (v2, _) = le(i64::from_le_bytes).parse(cursor).unwrap();

        assert_eq!(v1, 42);
        assert_eq!(v2, 123);
    }
}
