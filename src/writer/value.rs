//! Value serialization

use crate::types::{Value, ValueTag};
use std::io::{self, Write};

use super::array::write_array;
use super::file::write_file;

/// Write a value (consumes it to handle file handles)
pub fn write_value<W: Write>(writer: &mut W, value: Value) -> io::Result<()> {
    match value {
        Value::Bool(b) => {
            writer.write_all(&[ValueTag::Bool as u8])?;
            writer.write_all(&[if b { 1 } else { 0 }])?;
        }
        Value::Int(i) => {
            writer.write_all(&[ValueTag::Int as u8])?;
            writer.write_all(&i.to_le_bytes())?;
        }
        Value::Float(f) => {
            writer.write_all(&[ValueTag::Float as u8])?;
            writer.write_all(&f.to_le_bytes())?;
        }
        Value::String(s) => {
            writer.write_all(&[ValueTag::String as u8])?;
            let bytes = s.as_bytes();
            writer.write_all(&(bytes.len() as u32).to_le_bytes())?;
            writer.write_all(bytes)?;
        }
        Value::Array(arr) => {
            write_array(writer, &arr)?;
        }
        Value::File(file) => {
            write_file(writer, file)?;
        }
        Value::Object(entries) => {
            writer.write_all(&[ValueTag::Object as u8])?;
            writer.write_all(&(entries.len() as u32).to_le_bytes())?;
            for (key, val) in entries {
                if key.contains('/') {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidInput,
                        format!("key contains '/': {:?}", key),
                    ));
                }
                let key_bytes = key.as_bytes();
                writer.write_all(&(key_bytes.len() as u16).to_le_bytes())?;
                writer.write_all(key_bytes)?;
                write_value(writer, val)?;
            }
        }
        Value::List(items) => {
            writer.write_all(&[ValueTag::List as u8])?;
            writer.write_all(&(items.len() as u32).to_le_bytes())?;
            for item in items {
                write_value(writer, item)?;
            }
        }
    }
    Ok(())
}
