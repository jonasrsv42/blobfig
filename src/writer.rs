//! Serialization of blobfig values

mod aligned;
mod array;
mod file;
mod value;

use crate::error::{LengthField, WriteError};
use crate::types::{MAGIC, VERSION, Value};
use std::io::Write;

use aligned::AlignedWriter;
use value::write_value;

/// A `usize` length narrowed to the fixed-width integer the wire uses for a
/// count or length prefix. A value too large to represent is an error, not a
/// silent truncation into corrupt output.
fn wire_len<T: TryFrom<usize>>(len: usize, field: LengthField) -> Result<T, WriteError> {
    T::try_from(len).map_err(|_| WriteError::TooLarge { field, len })
}

/// Write a blobfig value to a writer (consumes the value to handle streaming)
pub fn write<W: Write>(writer: &mut W, value: Value) -> Result<(), WriteError> {
    // Array and file data are aligned from the start of the blob
    let writer = &mut AlignedWriter::new(writer);
    // Write header
    writer.write_all(MAGIC)?;
    writer.write_all(&VERSION.to_le_bytes())?;
    writer.write_all(&0u32.to_le_bytes())?; // flags (reserved)

    // Write the value
    write_value(writer, value)?;

    Ok(())
}

/// Write a blobfig value to bytes
pub fn to_bytes(value: Value) -> Result<Vec<u8>, WriteError> {
    let mut buf = Vec::new();
    write(&mut buf, value)?;
    Ok(buf)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{Array, DType, File};

    #[test]
    fn test_write_primitives() {
        let value = Value::object(vec![
            ("bool".to_string(), Value::Bool(true)),
            ("int".to_string(), Value::Int(42)),
            ("float".to_string(), Value::Float(2.5)),
            ("string".to_string(), Value::String("hello".to_string())),
        ]);

        let bytes = to_bytes(value).unwrap();

        // Check header
        assert_eq!(&bytes[0..8], MAGIC);
        assert_eq!(
            u32::from_le_bytes(bytes[8..12].try_into().unwrap()),
            VERSION
        );
    }

    #[test]
    fn test_write_nested() {
        let value = Value::object(vec![(
            "outer".to_string(),
            Value::object(vec![("inner".to_string(), Value::Int(123))]),
        )]);

        let bytes = to_bytes(value).unwrap();
        assert!(!bytes.is_empty());
    }

    #[test]
    fn test_write_list() {
        let value = Value::list(vec![Value::Int(1), Value::Int(2), Value::Int(3)]);

        let bytes = to_bytes(value).unwrap();
        assert!(!bytes.is_empty());
    }

    #[test]
    fn test_write_array() {
        let arr = Array::new(DType::F32, vec![2, 3], vec![0u8; 24]);
        let value = Value::Array(arr);

        let bytes = to_bytes(value).unwrap();
        assert!(!bytes.is_empty());
    }

    #[test]
    fn test_write_file_bytes() {
        let file = File::from_bytes("text/plain", b"hello world".to_vec());
        let value = Value::File(file);

        let bytes = to_bytes(value).unwrap();
        assert!(!bytes.is_empty());
    }

    #[test]
    fn test_write_with_file_handle() {
        use crate::types::FileHandle;
        use std::io::Cursor;

        struct CursorHandle {
            cursor: Cursor<Vec<u8>>,
            size: u64,
        }

        impl std::io::Read for CursorHandle {
            fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
                self.cursor.read(buf)
            }
        }

        impl FileHandle for CursorHandle {
            fn size(&self) -> u64 {
                self.size
            }
        }

        let data = b"streaming data";
        let handle = CursorHandle {
            cursor: Cursor::new(data.to_vec()),
            size: data.len() as u64,
        };
        let file = File::from_handle("text/plain", handle);
        let value = Value::File(file);

        let bytes = to_bytes(value).unwrap();
        assert!(!bytes.is_empty());
    }

    #[test]
    fn test_key_with_slash_rejected() {
        let value = Value::object(vec![("invalid/key".into(), Value::Int(1))]);
        let error = to_bytes(value).unwrap_err();
        assert!(matches!(error, WriteError::KeyContainsSlash { key } if key == "invalid/key"));
    }

    #[test]
    fn test_array_and_file_data_start_aligned_in_the_blob() {
        use crate::types::{ALIGNMENT, NodeView};
        let value = Value::object(vec![
            (
                "a".into(),
                Value::Array(Array::new(DType::U8, vec![3], vec![1, 2, 3])),
            ),
            (
                "file".into(),
                Value::File(File::from_bytes("text/plain", b"hi".to_vec())),
            ),
        ]);
        let bytes = to_bytes(value).unwrap();
        let parsed = crate::parse(&bytes).unwrap();
        let offset = |data: &[u8]| data.as_ptr() as usize - bytes.as_ptr() as usize;
        for data in [
            parsed.array("a").unwrap().data,
            parsed.file("file").unwrap().data,
        ] {
            assert_eq!(offset(data) % ALIGNMENT, 0, "data at {}", offset(data));
        }
    }

    #[test]
    fn test_short_writes_still_align() {
        // A writer that takes one byte per call, like a slow pipe or socket
        struct OneByte(Vec<u8>);
        impl Write for OneByte {
            fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
                self.0.extend_from_slice(&buf[..buf.len().min(1)]);
                Ok(buf.len().min(1))
            }
            fn flush(&mut self) -> std::io::Result<()> {
                Ok(())
            }
        }
        let value = || Value::Array(Array::new(DType::U8, vec![3], vec![1, 2, 3]));
        let mut out = OneByte(Vec::new());
        write(&mut out, value()).unwrap();
        assert_eq!(out.0, to_bytes(value()).unwrap());
    }

    #[test]
    fn test_writes_only_what_the_parser_can_read_deep() {
        // `depth` values deep: secrets, lists and objects in turn around a bool
        let nested = |depth: usize| {
            (1..depth).fold(Value::Bool(true), |v, level| match level % 3 {
                0 => Value::secret(v),
                1 => Value::list(vec![v]),
                _ => Value::object(vec![("k".into(), v)]),
            })
        };

        let bytes = to_bytes(nested(crate::MAX_DEPTH)).unwrap();
        assert!(crate::parse(&bytes).is_ok());

        let error = to_bytes(nested(crate::MAX_DEPTH + 1)).unwrap_err();
        assert!(matches!(error, WriteError::TooDeep));
    }

    #[test]
    fn test_duplicate_key_rejected() {
        let value = Value::object(vec![
            ("a".into(), Value::Int(1)),
            ("a".into(), Value::Int(2)),
        ]);
        let error = to_bytes(value).unwrap_err();
        assert!(
            matches!(error, WriteError::DuplicateKey { key, first: 0, second: 1 } if key == "a")
        );
    }
}
