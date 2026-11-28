//! Writing that knows where in the blob it is, so array and file data can be
//! padded to start aligned

use crate::types::{ALIGNMENT, padding_at};
use std::io::{self, Write};

/// Any writer, counting the bytes it accepts from the start of the blob, so
/// it can pad array and file data to start aligned
pub struct AlignedWriter<'sink, WriterType> {
    inner: &'sink mut WriterType,
    position: u64,
}

impl<'sink, WriterType: Write> AlignedWriter<'sink, WriterType> {
    /// `inner`, with the blob starting at the next byte written to it
    pub fn new(inner: &'sink mut WriterType) -> Self {
        AlignedWriter { inner, position: 0 }
    }

    /// Write zero bytes up to the next multiple of [`ALIGNMENT`]
    pub fn pad_to_alignment(&mut self) -> io::Result<()> {
        let padding = padding_at(self.position) as usize;
        self.write_all(&[0; ALIGNMENT][..padding])
    }
}

impl<WriterType: Write> Write for AlignedWriter<'_, WriterType> {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        let written = self.inner.write(buf)?;
        self.position += written as u64;
        Ok(written)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.inner.flush()
    }
}
