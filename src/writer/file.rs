//! File serialization with streaming support

use super::wire_len;
use crate::types::{File, FileData, ValueTag};
use std::io::{self, Read, Write};

/// Write a file value (handles streaming from handle)
pub fn write_file<W: Write>(writer: &mut W, mut file: File) -> io::Result<()> {
    writer.write_all(&[ValueTag::File as u8])?;

    let mimetype_bytes = file.mimetype.as_bytes();
    writer.write_all(&wire_len::<u16>(mimetype_bytes.len(), "mimetype")?.to_le_bytes())?;
    writer.write_all(mimetype_bytes)?;

    let size = file.size();
    writer.write_all(&size.to_le_bytes())?;

    match &mut file.data {
        FileData::Bytes(bytes) => {
            writer.write_all(bytes)?;
        }
        FileData::Handle(handle) => {
            stream_from_handle(writer, handle.as_mut(), size)?;
        }
    }

    Ok(())
}

/// Stream data from a handle to writer
fn stream_from_handle<W: Write, R: Read + ?Sized>(
    writer: &mut W,
    reader: &mut R,
    size: u64,
) -> io::Result<()> {
    let mut remaining = size;
    let mut buf = [0u8; 8192];

    while remaining > 0 {
        let to_read = std::cmp::min(remaining as usize, buf.len());
        let n = reader.read(&mut buf[..to_read])?;
        if n == 0 {
            return Err(io::Error::new(
                io::ErrorKind::UnexpectedEof,
                "Handle ended before expected size",
            ));
        }
        writer.write_all(&buf[..n])?;
        remaining -= n as u64;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::parse_value;
    use crate::types::NodeView;
    use parsicomb::{ByteCursor, Parser};

    #[test]
    fn mimetype_longer_than_u16_rejected() {
        // The mimetype length is a u16; 65536 used to be written as 0
        let file = File::from_bytes("x".repeat(65536), Vec::new());
        let error = write_file(&mut Vec::new(), file).unwrap_err();
        assert_eq!(error.kind(), io::ErrorKind::InvalidInput);
    }

    #[test]
    fn roundtrip_file_bytes() {
        let content = b"hello world";
        let file = File::from_bytes("text/plain", content.to_vec());

        let mut buf = Vec::new();
        write_file(&mut buf, file).unwrap();

        let cursor = ByteCursor::new(&buf);
        let (parsed, _) = parse_value().parse(cursor).unwrap();

        let f = parsed.as_file().unwrap();
        assert_eq!(f.mimetype, "text/plain");
        assert_eq!(f.data, content);
    }

    #[test]
    fn roundtrip_file_empty() {
        let file = File::from_bytes("application/octet-stream", vec![]);

        let mut buf = Vec::new();
        write_file(&mut buf, file).unwrap();

        let cursor = ByteCursor::new(&buf);
        let (parsed, _) = parse_value().parse(cursor).unwrap();

        let f = parsed.as_file().unwrap();
        assert_eq!(f.mimetype, "application/octet-stream");
        assert!(f.data.is_empty());
    }

    #[test]
    fn roundtrip_file_binary() {
        let content: Vec<u8> = (0..256).map(|i| i as u8).collect();
        let file = File::from_bytes("application/x-tflite", content.clone());

        let mut buf = Vec::new();
        write_file(&mut buf, file).unwrap();

        let cursor = ByteCursor::new(&buf);
        let (parsed, _) = parse_value().parse(cursor).unwrap();

        let f = parsed.as_file().unwrap();
        assert_eq!(f.mimetype, "application/x-tflite");
        assert_eq!(f.data, content.as_slice());
    }
}
