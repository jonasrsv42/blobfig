//! Header constants and structure

/// Magic bytes identifying a blobfig file
pub const MAGIC: &[u8; 8] = b"BLOBFIG\0";

/// Current format version
pub const VERSION: u32 = 1;

/// Header size in bytes (magic + version + flags)
pub const HEADER_SIZE: usize = 16;
