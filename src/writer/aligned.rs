//! Writing that knows where in the blob it is, so array and file data can be
//! padded to start aligned

use crate::error::WriteError;
use crate::types::{ALIGNMENT, padding_at};
use std::io::{self, Write};

/// Any writer, counting the bytes it accepts from the start of the blob, so
/// it can pad array and file data to start aligned
pub struct AlignedWriter<'w, W> {
    inner: &'w mut W,
    position: u64,
}

impl<'w, W: Write> AlignedWriter<'w, W> {
    /// `inner`, with the blob starting at the next byte written to it
    pub fn new(inner: &'w mut W) -> Self {
        AlignedWriter { inner, position: 0 }
    }

    /// Write zero bytes up to the next multiple of [`ALIGNMENT`]
    pub fn pad_to_alignment(&mut self) -> Result<(), WriteError> {
        let padding = padding_at(self.position) as usize;
        self.write_all(&[0; ALIGNMENT][..padding])?;
        Ok(())
    }
}

impl<W: Write> Write for AlignedWriter<'_, W> {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        let written = self.inner.write(buf)?;
        self.position += written as u64;
        Ok(written)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.inner.flush()
    }
}
