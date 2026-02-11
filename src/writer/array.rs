//! Array serialization

use crate::types::{Array, ValueTag};
use std::io::{self, Write};

/// Write an array value
pub fn write_array<W: Write>(writer: &mut W, arr: &Array) -> io::Result<()> {
    writer.write_all(&[ValueTag::Array as u8])?;
    writer.write_all(&[arr.dtype as u8])?;
    writer.write_all(&[arr.shape.len() as u8])?;
    for dim in &arr.shape {
        writer.write_all(&dim.to_le_bytes())?;
    }
    writer.write_all(&(arr.data.len() as u64).to_le_bytes())?;
    writer.write_all(&arr.data)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::parse_value;
    use crate::types::DType;
    use parsicomb::{ByteCursor, Parser};

    #[test]
    fn roundtrip_array_1d_u8() {
        let data = vec![1u8, 2, 3, 4];
        let arr = Array::new(DType::U8, vec![4], data.clone());

        let mut buf = Vec::new();
        write_array(&mut buf, &arr).unwrap();

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
        write_array(&mut buf, &arr).unwrap();

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
        write_array(&mut buf, &arr).unwrap();

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
        write_array(&mut buf, &arr).unwrap();

        let cursor = ByteCursor::new(&buf);
        let (parsed, _) = parse_value().parse(cursor).unwrap();

        let a = parsed.as_array().unwrap();
        assert_eq!(a.dtype, DType::F64);
        assert_eq!(a.shape, vec![0]);
        assert!(a.data.is_empty());
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
            let arr = Array::new(dtype, vec![8], data.clone());

            let mut buf = Vec::new();
            write_array(&mut buf, &arr).unwrap();

            let cursor = ByteCursor::new(&buf);
            let (parsed, _) = parse_value().parse(cursor).unwrap();

            let a = parsed.as_array().unwrap();
            assert_eq!(a.dtype, dtype);
        }
    }
}
