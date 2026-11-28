//! Value serialization

use super::aligned::AlignedWriter;
use crate::check::{MAX_DEPTH, check_keys};
use crate::error::{LengthField, PathSegment, WriteError, WriteErrorReason};
use crate::types::{List, Object, ObjectNode, Value, ValueTag};
use std::io::Write;

use super::array::write_array;
use super::file::write_file;
use super::wire_len;

/// Write a value, consuming it so each child is freed once written
pub fn write_value<WriterType: Write>(
    writer: &mut AlignedWriter<'_, WriterType>,
    value: Value,
) -> Result<(), WriteError> {
    write_owned(writer, value, 1)
}

/// Write a value that is `depth` values deep, counting itself; containers move
/// their children out, so each child is freed once written
fn write_owned<WriterType: Write>(
    writer: &mut AlignedWriter<'_, WriterType>,
    value: Value,
    depth: usize,
) -> Result<(), WriteError> {
    refuse_too_deep(depth)?;
    match value {
        Value::Object(object) => {
            write_object_header(writer, &object)?;
            for (key, val) in object {
                write_key(writer, &key)?;
                write_owned(writer, val, depth + 1)
                    .map_err(|error| error.inside(PathSegment::Key(key)))?;
            }
            Ok(())
        }
        Value::List(items) => {
            write_list_header(writer, &items)?;
            for (index, item) in items.into_iter().enumerate() {
                write_owned(writer, item, depth + 1)
                    .map_err(|error| error.inside(PathSegment::Index(index)))?;
            }
            Ok(())
        }
        // Leaves and secrets are written by reference and dropped here, a
        // secret wiping itself
        mut other => write_payload(writer, &mut other, depth),
    }
}

/// Write a value that is `depth` values deep without moving anything out of it
fn write_in_place<WriterType: Write>(
    writer: &mut AlignedWriter<'_, WriterType>,
    value: &mut Value,
    depth: usize,
) -> Result<(), WriteError> {
    refuse_too_deep(depth)?;
    write_payload(writer, value, depth)
}

/// Write a value's tag and payload, its children in place; `&mut` only to read
/// file handles
fn write_payload<WriterType: Write>(
    writer: &mut AlignedWriter<'_, WriterType>,
    value: &mut Value,
    depth: usize,
) -> Result<(), WriteError> {
    match value {
        Value::Bool(b) => {
            writer.write_all(&[ValueTag::Bool as u8])?;
            writer.write_all(&[if *b { 1 } else { 0 }])?;
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
        Value::Array(arr) => write_array(writer, arr)?,
        Value::File(file) => write_file(writer, file)?,
        Value::Object(object) => {
            write_object_header(writer, object)?;
            for (key, val) in object.iter_mut() {
                write_key(writer, key)?;
                write_in_place(writer, val, depth + 1)
                    .map_err(|error| error.inside(PathSegment::Key(key.clone())))?;
            }
        }
        Value::List(items) => {
            write_list_header(writer, items)?;
            for (index, item) in items.iter_mut().enumerate() {
                write_in_place(writer, item, depth + 1)
                    .map_err(|error| error.inside(PathSegment::Index(index)))?;
            }
        }
        Value::Secret(secret) => {
            // Secret is a transparent wrapper on the wire: tag + inner value.
            // The secrecy marker travels with the data so it survives a roundtrip.
            writer.write_all(&[ValueTag::Secret as u8])?;
            write_in_place(writer, secret.value_mut(), depth + 1)?;
        }
    }
    Ok(())
}

/// Deeper than [`MAX_DEPTH`] is refused, as the parser would refuse to read it
fn refuse_too_deep(depth: usize) -> Result<(), WriteError> {
    match depth > MAX_DEPTH {
        true => Err(WriteErrorReason::TooDeep.into()),
        false => Ok(()),
    }
}

/// Write an object's tag and entry count, refusing keys the parser would refuse
fn write_object_header<WriterType: Write>(
    writer: &mut AlignedWriter<'_, WriterType>,
    object: &Object,
) -> Result<(), WriteError> {
    check_keys(object.entries().map(|(key, _)| key))?;
    writer.write_all(&[ValueTag::Object as u8])?;
    writer.write_all(&wire_len::<u32>(object.len(), LengthField::Object)?.to_le_bytes())?;
    Ok(())
}

fn write_key<WriterType: Write>(
    writer: &mut AlignedWriter<'_, WriterType>,
    key: &str,
) -> Result<(), WriteError> {
    writer.write_all(&wire_len::<u16>(key.len(), LengthField::Key)?.to_le_bytes())?;
    writer.write_all(key.as_bytes())?;
    Ok(())
}

fn write_list_header<WriterType: Write>(
    writer: &mut AlignedWriter<'_, WriterType>,
    items: &List,
) -> Result<(), WriteError> {
    writer.write_all(&[ValueTag::List as u8])?;
    writer.write_all(&wire_len::<u32>(items.len(), LengthField::List)?.to_le_bytes())?;
    Ok(())
}
