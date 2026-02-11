//! Serialization of blobfig values

mod array;
mod file;
mod value;

use crate::types::{MAGIC, VERSION, Value};
use std::io::{self, Write};

use value::{compute_size, write_value};

/// Write a blobfig value to a writer (consumes the value to handle streaming)
pub fn write<W: Write>(writer: &mut W, value: Value) -> io::Result<()> {
    // Write header
    writer.write_all(MAGIC)?;
    writer.write_all(&VERSION.to_le_bytes())?;
    writer.write_all(&0u32.to_le_bytes())?; // flags (reserved)

    // Compute root size
    let root_size = compute_size(&value)?;
    writer.write_all(&root_size.to_le_bytes())?;

    // Write the value
    write_value(writer, value)?;

    Ok(())
}

/// Write a blobfig value to bytes
pub fn to_bytes(value: Value) -> io::Result<Vec<u8>> {
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
        let value = Value::Object(vec![
            ("bool".to_string(), Value::Bool(true)),
            ("int".to_string(), Value::Int(42)),
            ("float".to_string(), Value::Float(3.14)),
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
        let value = Value::Object(vec![(
            "outer".to_string(),
            Value::Object(vec![("inner".to_string(), Value::Int(123))]),
        )]);

        let bytes = to_bytes(value).unwrap();
        assert!(!bytes.is_empty());
    }

    #[test]
    fn test_write_list() {
        let value = Value::List(vec![Value::Int(1), Value::Int(2), Value::Int(3)]);

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
            fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
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
        let value = Value::Object(vec![("invalid/key".into(), Value::Int(1))]);
        let result = to_bytes(value);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains('/'));
    }

    #[test]
    fn test_nested_key_with_slash_rejected() {
        let value = Value::Object(vec![(
            "valid".into(),
            Value::Object(vec![("also/invalid".into(), Value::Int(1))]),
        )]);
        let result = to_bytes(value);
        assert!(result.is_err());
    }
}
