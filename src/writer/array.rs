//! Array serialization

use super::aligned::AlignedWriter;
use super::wire_len;
use crate::check::check_data_size;
use crate::error::{LengthField, WriteError};
use crate::types::{Array, ValueTag};
use std::io::Write;

/// Write an array value; its data must be the size its dtype and shape need
pub fn write_array<W: Write>(
    writer: &mut AlignedWriter<'_, W>,
    arr: &Array,
) -> Result<(), WriteError> {
    check_data_size(arr.dtype, &arr.shape, arr.data.len())?;
    writer.write_all(&[ValueTag::Array as u8])?;
    writer.write_all(&[arr.dtype as u8])?;
    writer.write_all(&[wire_len::<u8>(
        arr.shape.len(),
        LengthField::ArrayDimensions,
    )?])?;
    for dim in &arr.shape {
        writer.write_all(&dim.to_le_bytes())?;
    }
    writer.write_all(&(arr.data.len() as u64).to_le_bytes())?;
    writer.pad_to_alignment()?;
    writer.write_all(&arr.data)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::WriteErrorReason;
    use crate::parser::parse_value;
    use crate::types::{DType, NodeView};
    use parsicomb::{ByteCursor, Parser};

    #[test]
    fn more_than_255_dimensions_rejected() {
        // The dimension count is one byte; 256 used to be written as 0
        let arr = Array::new(DType::U8, vec![1; 256], vec![0]);
        let error = write_array(&mut AlignedWriter::new(&mut Vec::new()), &arr).unwrap_err();
        let expected = (LengthField::ArrayDimensions, 256);
        assert!(
            matches!(error.reason(), &WriteErrorReason::TooLarge { field, len } if (field, len) == expected)
        );
    }

    #[test]
    fn roundtrip_array_1d_u8() {
        let data = vec![1u8, 2, 3, 4];
        let arr = Array::new(DType::U8, vec![4], data.clone());

        let mut buf = Vec::new();
        write_array(&mut AlignedWriter::new(&mut buf), &arr).unwrap();

        let cursor = ByteCursor::new(&buf);
        let (parsed, _) = parse_value().parse(cursor).unwrap();

        let a = parsed.as_array().unwrap();
        assert_eq!(a.dtype, DType::U8);
        assert_eq!(a.shape, vec![4]);
        assert_eq!(a.data, data.as_slice());
    }

    #[test]
    fn roundtrip_array_2d_f32() {
        let values: Vec<f32> = vec![1.0, 2.0, 3.0, 4.0, 5.0, 6.0];
        let data: Vec<u8> = values.iter().flat_map(|f| f.to_le_bytes()).collect();
        let arr = Array::new(DType::F32, vec![2, 3], data.clone());

        let mut buf = Vec::new();
        write_array(&mut AlignedWriter::new(&mut buf), &arr).unwrap();

        let cursor = ByteCursor::new(&buf);
        let (parsed, _) = parse_value().parse(cursor).unwrap();

        let a = parsed.as_array().unwrap();
        assert_eq!(a.dtype, DType::F32);
        assert_eq!(a.shape, vec![2, 3]);
        assert_eq!(a.data, data.as_slice());
    }

    #[test]
    fn roundtrip_array_3d() {
        let data: Vec<u8> = (0..24).collect(); // 2x3x4 u8
        let arr = Array::new(DType::U8, vec![2, 3, 4], data.clone());

        let mut buf = Vec::new();
        write_array(&mut AlignedWriter::new(&mut buf), &arr).unwrap();

        let cursor = ByteCursor::new(&buf);
        let (parsed, _) = parse_value().parse(cursor).unwrap();

        let a = parsed.as_array().unwrap();
        assert_eq!(a.dtype, DType::U8);
        assert_eq!(a.shape, vec![2, 3, 4]);
        assert_eq!(a.data, data.as_slice());
    }

    #[test]
    fn roundtrip_array_empty() {
        let arr = Array::new(DType::F64, vec![0], vec![]);

        let mut buf = Vec::new();
        write_array(&mut AlignedWriter::new(&mut buf), &arr).unwrap();

        let cursor = ByteCursor::new(&buf);
        let (parsed, _) = parse_value().parse(cursor).unwrap();

        let a = parsed.as_array().unwrap();
        assert_eq!(a.dtype, DType::F64);
        assert_eq!(a.shape, vec![0]);
        assert!(a.data.is_empty());
    }

    #[test]
    fn data_size_must_match_dtype_and_shape() {
        let arr = Array::new(DType::F32, vec![1000], vec![0; 3]);
        let error = write_array(&mut AlignedWriter::new(&mut Vec::new()), &arr).unwrap_err();
        let expected = (Some(4000), 3);
        assert!(
            matches!(error.reason(), &WriteErrorReason::SizeMismatch { needed, actual, .. } if (needed, actual) == expected)
        );
    }

    #[test]
    fn roundtrip_all_dtypes() {
        for dtype in [
            DType::U8,
            DType::I8,
            DType::U16,
            DType::I16,
            DType::U32,
            DType::I32,
            DType::U64,
            DType::I64,
            DType::F32,
            DType::F64,
        ] {
            let data = vec![0u8; 8];
            let arr = Array::new(dtype, vec![8 / dtype.element_size() as u64], data.clone());

            let mut buf = Vec::new();
            write_array(&mut AlignedWriter::new(&mut buf), &arr).unwrap();

            let cursor = ByteCursor::new(&buf);
            let (parsed, _) = parse_value().parse(cursor).unwrap();

            let a = parsed.as_array().unwrap();
            assert_eq!(a.dtype, dtype);
        }
    }
}
