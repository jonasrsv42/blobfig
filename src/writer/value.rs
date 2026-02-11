//! Value serialization

use crate::types::{Value, ValueTag};
use std::io::{self, Write};

use super::array::{array_size, write_array};
use super::file::{file_size, write_file};

/// Compute the serialized size of a value
pub fn compute_size(value: &Value) -> io::Result<u64> {
    let size = match value {
        Value::Bool(_) => {
            let tag_size = 1u64;
            let value_size = 1u64;
            tag_size + value_size
        }
        Value::Int(_) => {
            let tag_size = 1u64;
            let value_size = 8u64; // i64
            tag_size + value_size
        }
        Value::Float(_) => {
            let tag_size = 1u64;
            let value_size = 8u64; // f64
            tag_size + value_size
        }
        Value::String(s) => {
            let tag_size = 1u64;
            let len_size = 4u64; // u32 for string length
            let data_size = s.len() as u64;
            tag_size + len_size + data_size
        }
        Value::Array(arr) => array_size(arr),
        Value::File(file) => file_size(file),
        Value::Object(entries) => {
            let tag_size = 1u64;
            let num_entries_size = 4u64; // u32 for entry count

            let mut entries_size = 0u64;
            for (key, val) in entries {
                let key_len_size = 2u64; // u16 for key length
                let key_size = key.len() as u64;
                let value_size = compute_size(val)?;
                entries_size += key_len_size + key_size + value_size;
            }

            tag_size + num_entries_size + entries_size
        }
        Value::List(items) => {
            let tag_size = 1u64;
            let num_items_size = 4u64; // u32 for item count

            let mut items_size = 0u64;
            for item in items {
                items_size += compute_size(item)?;
            }

            tag_size + num_items_size + items_size
        }
    };
    Ok(size)
}

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
