//! Array data size, shared by the parser and the writer

use crate::error::Reason;
use crate::types::DType;
use std::fmt;

/// Array data whose length isn't what its dtype and shape need
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct DataSizeError<'s> {
    pub dtype: DType,
    pub shape: &'s [u64],
    /// `None` when the shape is too large to fit in a `u64`
    pub needed: Option<u64>,
    pub actual: u64,
}

impl DataSizeError<'_> {
    pub fn reason(&self) -> Reason {
        Reason::SizeMismatch {
            needed: self.needed,
            actual: self.actual,
        }
    }
}

impl fmt::Display for DataSizeError<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let Self {
            dtype,
            shape,
            actual,
            ..
        } = self;
        write!(
            f,
            "array data is {actual} bytes, but {dtype:?} with shape {shape:?} needs "
        )?;
        match self.needed {
            Some(bytes) => write!(f, "{bytes} bytes"),
            None => write!(f, "more bytes than fit in a u64"),
        }
    }
}

/// Check that `actual` data bytes are what `dtype` with `shape` needs
pub(crate) fn check_data_size(
    dtype: DType,
    shape: &[u64],
    actual: usize,
) -> Result<(), DataSizeError<'_>> {
    let needed = dtype.data_size(shape);
    let actual = actual as u64;
    match needed == Some(actual) {
        true => Ok(()),
        false => Err(DataSizeError {
            dtype,
            shape,
            needed,
            actual,
        }),
    }
}
