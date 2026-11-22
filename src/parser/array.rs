//! Array parsing

use super::Failure;
use super::or_truncated::OrTruncatedExt;
use super::primitives::{checked, checked_byte, le};
use super::take::sized;
use crate::check::check_data_size;
use crate::error::Reason;
use crate::types::{ArrayView, DType};
use parsicomb::combinator::{AndExt, AndThenExt};
use parsicomb::{ByteCursor, Parser, ntimes};

/// Parse an array value (after tag has been consumed): a dtype byte, a u8
/// dimension count and that many u64 dimensions, then u64-length data
/// (zero-copy)
///
/// The data length must be what the dtype and shape need. Either field could
/// be the wrong one, so a mismatch is reported at the dtype byte, where the
/// array's fields start.
pub fn parse_array_body<'a>()
-> impl Parser<Cursor = ByteCursor<'a>, Output = ArrayView<'a>, Error = Failure<'a>> {
    let shape =
        le(u8::from_le_bytes).and_then(|ndim| ntimes(le(u64::from_le_bytes), usize::from(ndim)));
    let array = checked_byte(DType::from_u8, |found| Reason::BadDType { found })
        .and(shape)
        .and(sized(le(u64::from_le_bytes)))
        .or_truncated();
    checked(array, |((dtype, shape), data)| {
        check_data_size(dtype, &shape, data.len())?;
        Ok(ArrayView { dtype, shape, data })
    })
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
}
