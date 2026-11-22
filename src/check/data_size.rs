//! Array data size, shared by the parser and the writer

use crate::types::DType;

/// Check that `actual` data bytes are what `dtype` with `shape` needs; the
/// error message names all three
pub(crate) fn check_data_size(dtype: DType, shape: &[u64], actual: usize) -> Result<(), String> {
    let needed = dtype.data_size(shape);
    if needed == Some(actual as u64) {
        return Ok(());
    }
    let needed = match needed {
        Some(bytes) => format!("{bytes} bytes"),
        None => "more bytes than fit in a u64".to_string(),
    };
    Err(format!(
        "array data is {actual} bytes, but {dtype:?} with shape {shape:?} needs {needed}"
    ))
}
