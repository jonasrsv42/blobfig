//! Typed array types

use super::DType;
use super::node_view::ArrayNode;

/// Owned typed array (for building/writing)
#[derive(Debug, Clone)]
pub struct Array {
    pub dtype: DType,
    pub shape: Vec<u64>,
    pub data: Vec<u8>,
}

impl Array {
    pub fn new(dtype: DType, shape: Vec<u64>, data: Vec<u8>) -> Self {
        Self { dtype, shape, data }
    }

    /// Total number of elements
    pub fn num_elements(&self) -> u64 {
        self.shape.iter().product()
    }

    /// Expected data size in bytes, or `None` if it doesn't fit in a `u64`
    pub fn expected_size(&self) -> Option<u64> {
        self.dtype.data_size(&self.shape)
    }
}

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

/// View into a typed array stored in the blob (zero-copy)
#[derive(Debug, Clone)]
pub struct ArrayView<'a> {
    pub dtype: DType,
    pub shape: Vec<u64>,
    pub data: &'a [u8],
}

impl<'a> ArrayView<'a> {
    /// Total number of elements
    pub fn num_elements(&self) -> u64 {
        self.shape.iter().product()
    }

    /// Expected data size in bytes, or `None` if it doesn't fit in a `u64`
    pub fn expected_size(&self) -> Option<u64> {
        self.dtype.data_size(&self.shape)
    }

    /// Convert to owned Array
    pub fn to_owned(&self) -> Array {
        Array {
            dtype: self.dtype,
            shape: self.shape.clone(),
            data: self.data.to_vec(),
        }
    }
}

impl ArrayNode for Array {
    fn dtype(&self) -> DType {
        self.dtype
    }

    fn shape(&self) -> &[u64] {
        &self.shape
    }

    fn data(&self) -> &[u8] {
        &self.data
    }
}

impl ArrayNode for ArrayView<'_> {
    fn dtype(&self) -> DType {
        self.dtype
    }

    fn shape(&self) -> &[u64] {
        &self.shape
    }

    fn data(&self) -> &[u8] {
        self.data
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn owned_array_reports_dtype_shape_and_data() {
        let array = Array::new(DType::U8, vec![2, 2], vec![1, 2, 3, 4]);
        assert_eq!(array.dtype(), DType::U8);
        assert_eq!(array.shape(), &[2, 2]);
        assert_eq!(array.data(), &[1, 2, 3, 4]);
    }

    #[test]
    fn array_view_reports_dtype_shape_and_data() {
        let view = ArrayView {
            dtype: DType::I16,
            shape: vec![3],
            data: &[9, 8, 7, 6, 5, 4],
        };
        assert_eq!(view.dtype(), DType::I16);
        assert_eq!(view.shape(), &[3]);
        assert_eq!(view.data(), &[9, 8, 7, 6, 5, 4]);
    }
}
