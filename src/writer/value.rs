//! Value serialization

use super::aligned::AlignedWriter;
use crate::check::{MAX_DEPTH, check_keys};
use crate::error::{LengthField, PathSegment, WriteError, WriteErrorReason};
use crate::types::{Object, ObjectNode, Value, ValueTag};
use std::io::Write;

use super::array::write_array;
use super::file::write_file;
use super::wire_len;

/// Write a value (consumes it to handle file handles)
pub fn write_value<W: Write>(
    writer: &mut AlignedWriter<'_, W>,
    value: Value,
) -> Result<(), WriteError> {
    write_nested(writer, value, 1)
}

/// Write a value that is `depth` values deep, counting itself; deeper than
/// [`MAX_DEPTH`] is refused, as the parser would refuse to read it
fn write_nested<W: Write>(
    writer: &mut AlignedWriter<'_, W>,
    value: Value,
    depth: usize,
) -> Result<(), WriteError> {
    if depth > MAX_DEPTH {
        return Err(WriteErrorReason::TooDeep.into());
    }
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
            writer.write_all(&wire_len::<u32>(bytes.len(), LengthField::String)?.to_le_bytes())?;
            writer.write_all(bytes)?;
        }
        Value::Array(arr) => write_array(writer, &arr)?,
        Value::File(file) => write_file(writer, file)?,
        Value::Object(object) => write_object(writer, object, depth)?,
        Value::List(items) => {
            writer.write_all(&[ValueTag::List as u8])?;
            writer.write_all(&wire_len::<u32>(items.len(), LengthField::List)?.to_le_bytes())?;
            for (index, item) in items.into_iter().enumerate() {
                write_nested(writer, item, depth + 1)
                    .map_err(|error| error.inside(PathSegment::Index(index)))?;
            }
        }
        Value::Secret(inner) => {
            // Secret is a transparent wrapper on the wire: tag + inner value.
            // The secrecy marker travels with the data so it survives a roundtrip.
            writer.write_all(&[ValueTag::Secret as u8])?;
            write_nested(writer, *inner, depth + 1)?;
        }
    }
    Ok(())
}

/// Write an object `depth` values deep, refusing keys the parser would refuse
fn write_object<W: Write>(
    writer: &mut AlignedWriter<'_, W>,
    object: Object,
    depth: usize,
) -> Result<(), WriteError> {
    check_keys(object.entries().map(|(key, _)| key))?;
    writer.write_all(&[ValueTag::Object as u8])?;
    writer.write_all(&wire_len::<u32>(object.len(), LengthField::Object)?.to_le_bytes())?;
    for (key, val) in object {
        writer.write_all(&wire_len::<u16>(key.len(), LengthField::Key)?.to_le_bytes())?;
        writer.write_all(key.as_bytes())?;
        write_nested(writer, val, depth + 1)
            .map_err(|error| error.inside(PathSegment::Key(key)))?;
    }
    Ok(())
}
