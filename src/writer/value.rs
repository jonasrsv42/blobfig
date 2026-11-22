//! Value serialization

use crate::check::check_keys;
use crate::types::{ObjectNode, Value, ValueTag};
use std::io::{self, Write};

use super::array::write_array;
use super::file::write_file;
use super::wire_len;

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
            writer.write_all(&wire_len::<u32>(bytes.len(), "string")?.to_le_bytes())?;
            writer.write_all(bytes)?;
        }
        Value::Array(arr) => {
            write_array(writer, &arr)?;
        }
        Value::File(file) => {
            write_file(writer, file)?;
        }
        Value::Object(entries) => {
            check_keys(entries.entries().map(|(key, _)| key))
                .map_err(|error| io::Error::new(io::ErrorKind::InvalidInput, error.to_string()))?;
            writer.write_all(&[ValueTag::Object as u8])?;
            writer.write_all(&wire_len::<u32>(entries.len(), "object")?.to_le_bytes())?;
            for (key, val) in entries {
                let key_bytes = key.as_bytes();
                writer.write_all(&wire_len::<u16>(key_bytes.len(), "key")?.to_le_bytes())?;
                writer.write_all(key_bytes)?;
                write_value(writer, val)?;
            }
        }
        Value::List(items) => {
            writer.write_all(&[ValueTag::List as u8])?;
            writer.write_all(&wire_len::<u32>(items.len(), "list")?.to_le_bytes())?;
            for item in items {
                write_value(writer, item)?;
            }
        }
        Value::Secret(inner) => {
            // Secret is a transparent wrapper on the wire: tag + inner value.
            // The secrecy marker travels with the data so it survives a roundtrip.
            writer.write_all(&[ValueTag::Secret as u8])?;
            write_value(writer, *inner)?;
        }
    }
    Ok(())
}
