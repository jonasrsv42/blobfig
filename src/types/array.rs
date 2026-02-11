//! Typed array types

use super::DType;

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

    /// Expected data size in bytes
    pub fn expected_size(&self) -> u64 {
        self.num_elements() * self.dtype.element_size() as u64
    }
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

    /// Expected data size in bytes
    pub fn expected_size(&self) -> u64 {
        self.num_elements() * self.dtype.element_size() as u64
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
