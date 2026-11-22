//! File blob parsing

use super::Failure;
use super::or_truncated::OrTruncatedExt;
use super::primitives::le;
use super::string::utf8;
use super::take::aligned;
use crate::types::FileView;
use parsicomb::combinator::AndExt;
use parsicomb::{ByteCursor, MapExt, Parser};

/// Parse a file value (after tag has been consumed): a u16-length mimetype,
/// then a u64 length, zero padding to [`ALIGNMENT`](crate::ALIGNMENT), and the
/// data (zero-copy)
pub fn parse_file_body<'a>()
-> impl Parser<Cursor = ByteCursor<'a>, Output = FileView<'a>, Error = Failure<'a>> {
    utf8(le(u16::from_le_bytes), "mimetype")
        .and(aligned(le(u64::from_le_bytes)))
        .map(|(mimetype, data)| FileView { mimetype, data })
        .or_truncated()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::ALIGNMENT;

    fn make_file_bytes(mimetype: &str, data: &[u8]) -> Vec<u8> {
        let mut bytes = Vec::new();
        bytes.extend_from_slice(&(mimetype.len() as u16).to_le_bytes());
        bytes.extend_from_slice(mimetype.as_bytes());
        bytes.extend_from_slice(&(data.len() as u64).to_le_bytes());
        bytes.resize(bytes.len().next_multiple_of(ALIGNMENT), 0);
        bytes.extend_from_slice(data);
        bytes
    }

    #[test]
    fn test_parse_file() {
        let data = b"hello world";
        let bytes = make_file_bytes("text/plain", data);
        let cursor = ByteCursor::new(&bytes);

        let (file, _) = parse_file_body().parse(cursor).unwrap();
        assert_eq!(file.mimetype, "text/plain");
        assert_eq!(file.data, b"hello world");
    }

    #[test]
    fn test_parse_file_empty() {
        let bytes = make_file_bytes("application/octet-stream", &[]);
        let cursor = ByteCursor::new(&bytes);

        let (file, _) = parse_file_body().parse(cursor).unwrap();
        assert_eq!(file.mimetype, "application/octet-stream");
        assert_eq!(file.data, &[]);
    }

    #[test]
    fn test_parse_file_zero_copy() {
        let data = b"test data";
        let bytes = make_file_bytes("text/plain", data);
        let cursor = ByteCursor::new(&bytes);

        let (file, _) = parse_file_body().parse(cursor).unwrap();

        // Verify data points into original bytes
        // ...after the padding, at the first multiple of ALIGNMENT
        assert!(std::ptr::eq(
            file.data.as_ptr(),
            bytes[ALIGNMENT..].as_ptr()
        ));
    }

    #[test]
    fn test_parse_file_binary() {
        let data: Vec<u8> = (0..256).map(|i| i as u8).collect();
        let bytes = make_file_bytes("application/x-tflite", &data);
        let cursor = ByteCursor::new(&bytes);

        let (file, _) = parse_file_body().parse(cursor).unwrap();
        assert_eq!(file.mimetype, "application/x-tflite");
        assert_eq!(file.data.len(), 256);
    }
}
