//! Header constants and structure

/// Magic bytes identifying a blobfig file
pub const MAGIC: &[u8; 8] = b"BLOBFIG\0";

/// Current format version
///
/// Once blobs outlive the code that wrote them, a change older readers can't
/// read, such as a new value tag or dtype, bumps it, so they refuse a newer
/// blob as unsupported instead of reading it as corrupt.
pub const VERSION: u32 = 1;

/// Header size in bytes (magic + version + flags)
pub const HEADER_SIZE: usize = 16;
