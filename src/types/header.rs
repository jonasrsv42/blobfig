//! Header constants and structure

/// Magic bytes identifying a blobfig file
pub const MAGIC: &[u8; 8] = b"BLOBFIG\0";

/// Current format version
///
/// Once blobs outlive the code that wrote them, a change older readers can't
/// read, such as a new value tag or dtype, bumps it, so they refuse a newer
/// blob as unsupported instead of reading it as corrupt.
pub const VERSION: u32 = 1;

/// Array and file data start at an offset into the blob that is a multiple of
/// this, after zero padding
///
/// An mmap'd blob (or any buffer that starts 64-byte aligned) can then be read
/// as typed arrays without copying, and a file such as a model sits aligned
/// for its own loader. 64 bytes covers every dtype, SIMD loads and a cache
/// line.
pub const ALIGNMENT: usize = 64;

/// How many zero bytes come at `position` in a blob before aligned data: up
/// to the next multiple of [`ALIGNMENT`]
pub(crate) fn padding_at(position: u64) -> u64 {
    position.next_multiple_of(ALIGNMENT as u64) - position
}

/// Header size in bytes (magic + version + flags; no flags are defined yet, so
/// they are 0)
pub const HEADER_SIZE: usize = 16;
