//! Primitive binary parsers built from combinators

use super::take::take;
use parsicomb::map::MapExt;
use parsicomb::{ByteCursor, Parser, ParsicombError, byte::byte};

/// Parse a u8
pub fn u8_parser<'a>()
-> impl Parser<'a, Cursor = ByteCursor<'a>, Output = u8, Error = ParsicombError<'a>> {
    byte()
}

/// Parse a u16 (little-endian)
pub fn u16_le<'a>()
-> impl Parser<'a, Cursor = ByteCursor<'a>, Output = u16, Error = ParsicombError<'a>> {
    take(2).map(|bytes: &[u8]| u16::from_le_bytes([bytes[0], bytes[1]]))
}

/// Parse a u32 (little-endian)
pub fn u32_le<'a>()
-> impl Parser<'a, Cursor = ByteCursor<'a>, Output = u32, Error = ParsicombError<'a>> {
    take(4).map(|bytes: &[u8]| u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]))
}

/// Parse a u64 (little-endian)
pub fn u64_le<'a>()
-> impl Parser<'a, Cursor = ByteCursor<'a>, Output = u64, Error = ParsicombError<'a>> {
    take(8).map(|bytes: &[u8]| {
        u64::from_le_bytes([
            bytes[0], bytes[1], bytes[2], bytes[3], bytes[4], bytes[5], bytes[6], bytes[7],
        ])
    })
}

/// Parse an i64 (little-endian)
pub fn i64_le<'a>()
-> impl Parser<'a, Cursor = ByteCursor<'a>, Output = i64, Error = ParsicombError<'a>> {
    take(8).map(|bytes: &[u8]| {
        i64::from_le_bytes([
            bytes[0], bytes[1], bytes[2], bytes[3], bytes[4], bytes[5], bytes[6], bytes[7],
        ])
    })
}

/// Parse an f64 (little-endian)
pub fn f64_le<'a>()
-> impl Parser<'a, Cursor = ByteCursor<'a>, Output = f64, Error = ParsicombError<'a>> {
    take(8).map(|bytes: &[u8]| {
        f64::from_le_bytes([
            bytes[0], bytes[1], bytes[2], bytes[3], bytes[4], bytes[5], bytes[6], bytes[7],
        ])
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_u8() {
        let data = &[0x42, 0x43];
        let cursor = ByteCursor::new(data);
        let (val, cursor) = u8_parser().parse(cursor).unwrap();
        assert_eq!(val, 0x42);
        let (val, _) = u8_parser().parse(cursor).unwrap();
        assert_eq!(val, 0x43);
    }

    #[test]
    fn test_u16_le() {
        let data = &[0x01, 0x02]; // 0x0201 in LE
        let cursor = ByteCursor::new(data);
        let (val, _) = u16_le().parse(cursor).unwrap();
        assert_eq!(val, 0x0201);
    }

    #[test]
    fn test_u32_le() {
        let data = &[0x01, 0x02, 0x03, 0x04];
        let cursor = ByteCursor::new(data);
        let (val, _) = u32_le().parse(cursor).unwrap();
        assert_eq!(val, 0x04030201);
    }

    #[test]
    fn test_u64_le() {
        let data = &[0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08];
        let cursor = ByteCursor::new(data);
        let (val, _) = u64_le().parse(cursor).unwrap();
        assert_eq!(val, 0x0807060504030201);
    }

    #[test]
    fn test_i64_le() {
        let data = (-42i64).to_le_bytes();
        let cursor = ByteCursor::new(&data);
        let (val, _) = i64_le().parse(cursor).unwrap();
        assert_eq!(val, -42);
    }

    #[test]
    fn test_f64_le() {
        let data = (3.14f64).to_le_bytes();
        let cursor = ByteCursor::new(&data);
        let (val, _) = f64_le().parse(cursor).unwrap();
        assert!((val - 3.14).abs() < 1e-10);
    }

    #[test]
    fn test_chained() {
        let mut data = Vec::new();
        data.extend_from_slice(&42u32.to_le_bytes());
        data.extend_from_slice(&123i64.to_le_bytes());

        let cursor = ByteCursor::new(&data);
        let (v1, cursor) = u32_le().parse(cursor).unwrap();
        let (v2, _) = i64_le().parse(cursor).unwrap();

        assert_eq!(v1, 42);
        assert_eq!(v2, 123);
    }
}
