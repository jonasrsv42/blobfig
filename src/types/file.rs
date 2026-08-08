//! File blob types

use std::io::Read;

use super::node_view::FileNode;

/// Trait for file data sources that can be read and have known size
pub trait FileHandle: Read + Send {
    /// Total size in bytes
    fn size(&self) -> u64;
}

/// Source of file data - either in-memory or from a handle
pub enum FileData {
    /// In-memory bytes
    Bytes(Vec<u8>),
    /// Handle implementing FileHandle trait
    Handle(Box<dyn FileHandle>),
}

impl std::fmt::Debug for FileData {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            FileData::Bytes(b) => f.debug_tuple("Bytes").field(&b.len()).finish(),
            FileData::Handle(h) => f.debug_struct("Handle").field("size", &h.size()).finish(),
        }
    }
}

impl FileData {
    /// Get the size of the data
    pub fn size(&self) -> u64 {
        match self {
            FileData::Bytes(b) => b.len() as u64,
            FileData::Handle(h) => h.size(),
        }
    }
}

/// Owned file blob (for building/writing)
#[derive(Debug)]
pub struct File {
    pub mimetype: String,
    pub data: FileData,
}

impl File {
    /// Create from in-memory bytes
    pub fn from_bytes(mimetype: impl Into<String>, data: Vec<u8>) -> Self {
        Self {
            mimetype: mimetype.into(),
            data: FileData::Bytes(data),
        }
    }

    /// Create from a FileHandle
    pub fn from_handle(mimetype: impl Into<String>, handle: impl FileHandle + 'static) -> Self {
        Self {
            mimetype: mimetype.into(),
            data: FileData::Handle(Box::new(handle)),
        }
    }

    /// Get the size of the file data
    pub fn size(&self) -> u64 {
        self.data.size()
    }
}

/// View into a file blob stored in the blob (zero-copy)
#[derive(Debug, Clone, Copy)]
pub struct FileView<'a> {
    pub mimetype: &'a str,
    pub data: &'a [u8],
}

impl<'a> FileView<'a> {
    /// Convert to owned File (in-memory)
    pub fn to_owned(&self) -> File {
        File {
            mimetype: self.mimetype.to_string(),
            data: FileData::Bytes(self.data.to_vec()),
        }
    }
}

impl FileNode for File {
    fn mimetype(&self) -> &str {
        self.mimetype.as_str()
    }

    fn size(&self) -> u64 {
        self.data.size()
    }
}

impl FileNode for FileView<'_> {
    fn mimetype(&self) -> &str {
        self.mimetype
    }

    fn size(&self) -> u64 {
        self.data.len() as u64
    }
}

#[cfg(test)]
mod tests {
    use std::io::Cursor;

    use super::*;

    /// A minimal in-memory [`FileHandle`] for exercising the handle-backed
    /// `File` path (its size is known; its bytes stream on read).
    struct BytesHandle(Cursor<Vec<u8>>);

    impl Read for BytesHandle {
        fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
            self.0.read(buf)
        }
    }

    impl FileHandle for BytesHandle {
        fn size(&self) -> u64 {
            self.0.get_ref().len() as u64
        }
    }

    #[test]
    fn file_view_reports_mimetype_and_size() {
        let view = FileView {
            mimetype: "text/plain",
            data: b"hello",
        };
        assert_eq!(view.mimetype(), "text/plain");
        assert_eq!(view.size(), 5);
    }

    #[test]
    fn in_memory_file_reports_mimetype_and_size() {
        let file = File::from_bytes("application/octet-stream", b"world!".to_vec());
        assert_eq!(file.mimetype(), "application/octet-stream");
        assert_eq!(file.size(), 6);
    }

    #[test]
    fn handle_backed_file_reports_its_streamed_size() {
        let file = File::from_handle("model/gltf", BytesHandle(Cursor::new(b"streamed".to_vec())));
        assert_eq!(file.mimetype(), "model/gltf");
        assert_eq!(file.size(), 8);
    }
}
