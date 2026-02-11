//! File blob types

use std::io::Read;

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
