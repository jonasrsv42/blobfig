//! Fixed-width fields and checks: little-endian numbers, checked values

use parsicomb::byte::byte;
use parsicomb::{ByteCursor, Cursor, Parser, ParsicombError, parser_fn};
use std::borrow::Cow;

/// Run `parser`, then `check` its output; a refused output is reported at
/// the start of what `parser` read, with `check`'s message
pub fn checked<'a, T, U>(
    parser: impl Parser<Cursor = ByteCursor<'a>, Output = T, Error = ParsicombError<'a>>,
    check: impl Fn(T) -> Result<U, String>,
) -> impl Parser<Cursor = ByteCursor<'a>, Output = U, Error = ParsicombError<'a>> {
    parser_fn(move |cursor: ByteCursor<'a>| {
        let (value, next) = parser.parse(cursor)?;
        match check(value) {
            Ok(value) => Ok((value, next)),
            Err(message) => Err(cursor.invalid_error(Cow::Owned(message))),
        }
    })
}

/// Read one byte and convert it with `convert`; a byte it refuses (`None`) is
/// reported at that byte as "invalid `what`: 0x..", e.g.
/// `checked_byte(DType::from_u8, "dtype")`
pub fn checked_byte<'a, T>(
    convert: fn(u8) -> Option<T>,
    what: &'static str,
) -> impl Parser<Cursor = ByteCursor<'a>, Output = T, Error = ParsicombError<'a>> {
    checked(byte(), move |b| {
        convert(b).ok_or_else(|| format!("invalid {what}: 0x{b:02X}"))
    })
}

/// Parse `N` bytes as a little-endian number, converted with `from`, e.g.
/// `le(u32::from_le_bytes)`
pub fn le<'a, const N: usize, T>(
    from: fn([u8; N]) -> T,
) -> impl Parser<Cursor = ByteCursor<'a>, Output = T, Error = ParsicombError<'a>> {
    parser_fn(move |cursor: ByteCursor<'a>| {
        let (data, pos) = cursor.inner();
        match data.get(pos..).and_then(<[u8]>::first_chunk::<N>) {
            Some(chunk) => Ok((from(*chunk), cursor.advance(N))),
            None => Err(ParsicombError::EndOfInput(cursor.loc())),
        }
    })
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

    #[test]
    fn test_short_input_is_end_of_input() {
        let data = &[0x01, 0x02, 0x03];
        let error = le(u32::from_le_bytes)
            .parse(ByteCursor::new(data))
            .unwrap_err();
        assert!(matches!(error, ParsicombError::EndOfInput(_)));
        assert_eq!(error.position(), 0);
    }
}
