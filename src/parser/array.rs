//! Array parsing

use super::primitives::{u8_parser, u64_le};
use super::take::take;
use crate::types::{ArrayView, DType};
use parsicomb::{ByteCursor, CodeLoc, Cursor, Parser, ParsicombError, ntimes};
use std::borrow::Cow;

/// Parse an array value (after tag has been consumed)
pub fn parse_array_body<'a>()
-> impl Parser<'a, Cursor = ByteCursor<'a>, Output = ArrayView<'a>, Error = ParsicombError<'a>> {
    ArrayBodyParser
}

struct ArrayBodyParser;

impl<'a> Parser<'a> for ArrayBodyParser {
    type Cursor = ByteCursor<'a>;
    type Output = ArrayView<'a>;
    type Error = ParsicombError<'a>;

    fn parse(&self, cursor: Self::Cursor) -> Result<(Self::Output, Self::Cursor), Self::Error> {
        // Parse dtype
        let (dtype_byte, cursor) = u8_parser().parse(cursor)?;
        let dtype = DType::from_u8(dtype_byte).ok_or_else(|| {
            let (data, pos) = cursor.inner();
            ParsicombError::SyntaxError {
                message: Cow::Owned(format!("Invalid dtype: 0x{:02X}", dtype_byte)),
                loc: CodeLoc::new(data, pos.saturating_sub(1)),
            }
        })?;

        // Parse ndim
        let (ndim, cursor) = u8_parser().parse(cursor)?;

        // Parse shape (ndim u64 values)
        let (shape, cursor) = ntimes(ndim as usize, u64_le()).parse(cursor)?;

        // Parse data size
        let (data_size, cursor) = u64_le().parse(cursor)?;

        // Take data bytes (zero-copy)
        let (data, cursor) = take(data_size as usize).parse(cursor)?;

        Ok((ArrayView { dtype, shape, data }, cursor))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_array_bytes(dtype: DType, shape: &[u64], data: &[u8]) -> Vec<u8> {
        let mut bytes = Vec::new();
        bytes.push(dtype as u8);
        bytes.push(shape.len() as u8);
        for dim in shape {
            bytes.extend_from_slice(&dim.to_le_bytes());
        }
        bytes.extend_from_slice(&(data.len() as u64).to_le_bytes());
        bytes.extend_from_slice(data);
        bytes
    }

    #[test]
    fn test_parse_array_1d() {
        let data = vec![1u8, 2, 3, 4];
        let bytes = make_array_bytes(DType::U8, &[4], &data);
        let cursor = ByteCursor::new(&bytes);

        let (arr, _) = parse_array_body().parse(cursor).unwrap();
        assert_eq!(arr.dtype, DType::U8);
        assert_eq!(arr.shape, vec![4]);
        assert_eq!(arr.data, &[1, 2, 3, 4]);
    }

    #[test]
    fn test_parse_array_2d() {
        let data = vec![0u8; 24]; // 2x3 f32 = 24 bytes
        let bytes = make_array_bytes(DType::F32, &[2, 3], &data);
        let cursor = ByteCursor::new(&bytes);

        let (arr, _) = parse_array_body().parse(cursor).unwrap();
        assert_eq!(arr.dtype, DType::F32);
        assert_eq!(arr.shape, vec![2, 3]);
        assert_eq!(arr.data.len(), 24);
    }

    #[test]
    fn test_parse_array_zero_copy() {
        let data = vec![1u8, 2, 3, 4];
        let bytes = make_array_bytes(DType::U8, &[4], &data);
        let cursor = ByteCursor::new(&bytes);

        let (arr, _) = parse_array_body().parse(cursor).unwrap();

        // Verify data points into original bytes
        let data_offset = 1 + 1 + 8 + 8; // dtype + ndim + shape + data_len
        assert!(std::ptr::eq(
            arr.data.as_ptr(),
            bytes[data_offset..].as_ptr()
        ));
    }

    #[test]
    fn test_invalid_dtype() {
        let mut bytes = vec![0xFF]; // Invalid dtype
        bytes.push(1); // ndim
        bytes.extend_from_slice(&1u64.to_le_bytes()); // shape
        bytes.extend_from_slice(&0u64.to_le_bytes()); // data_len

        let cursor = ByteCursor::new(&bytes);
        let result = parse_array_body().parse(cursor);
        assert!(result.is_err());
    }
}
