//! File blob parsing

use super::primitives::{u16_le, u64_le};
use super::string::length_prefixed_str;
use super::take::take;
use crate::types::FileView;
use parsicomb::MapExt;
use parsicomb::{ByteCursor, Parser, ParsicombError, parser_fn};

/// Parse a file value (after tag has been consumed)
pub fn parse_file_body<'a>()
-> impl Parser<Cursor = ByteCursor<'a>, Output = FileView<'a>, Error = ParsicombError<'a>> {
    let mimetype = length_prefixed_str(
        u16_le().map(|len| len as usize),
        "Invalid UTF-8 in mimetype",
    );
    parser_fn(move |cursor: ByteCursor<'a>| {
        let (mimetype, cursor) = mimetype.parse(cursor)?;
        let (data_size, cursor) = u64_le().parse(cursor)?;
        // Data bytes, zero-copy
        let (data, cursor) = take(data_size as usize).parse(cursor)?;
        Ok((FileView { mimetype, data }, cursor))
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_file_bytes(mimetype: &str, data: &[u8]) -> Vec<u8> {
        let mut bytes = Vec::new();
        bytes.extend_from_slice(&(mimetype.len() as u16).to_le_bytes());
        bytes.extend_from_slice(mimetype.as_bytes());
        bytes.extend_from_slice(&(data.len() as u64).to_le_bytes());
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
        let mimetype = "text/plain";
        let data_offset = 2 + mimetype.len() + 8; // mimetype_len + mimetype + data_len
        assert!(std::ptr::eq(
            file.data.as_ptr(),
            bytes[data_offset..].as_ptr()
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
