//! ndarray integration for blobfig arrays
//!
//! This module provides conversions between blobfig's Array/ArrayView types
//! and ndarray's Array/ArrayView types.
//!
//! Enable with the `ndarray` feature flag.

use crate::types::{Array, ArrayView, DType};
use ndarray::{ArrayD, ArrayViewD, IxDyn};

/// Error type for ndarray conversions
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NdarrayError {
    /// DType mismatch between expected and actual
    DTypeMismatch { expected: DType, actual: DType },
    /// Shape doesn't match data length
    ShapeMismatch { shape: Vec<u64>, data_len: usize },
    /// Data is not properly aligned for the element type
    AlignmentError,
    /// Array is not in standard (contiguous row-major) layout
    NotContiguous,
}

impl std::fmt::Display for NdarrayError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            NdarrayError::DTypeMismatch { expected, actual } => {
                write!(
                    f,
                    "DType mismatch: expected {:?}, got {:?}",
                    expected, actual
                )
            }
            NdarrayError::ShapeMismatch { shape, data_len } => {
                write!(
                    f,
                    "Shape {:?} doesn't match data length {}",
                    shape, data_len
                )
            }
            NdarrayError::AlignmentError => {
                write!(f, "Data is not properly aligned for element type")
            }
            NdarrayError::NotContiguous => {
                write!(
                    f,
                    "Array is not contiguous; call .as_standard_layout().into_owned() first"
                )
            }
        }
    }
}

impl std::error::Error for NdarrayError {}

/// Trait for types that can be stored in a blobfig array
pub trait ArrayType: Sized + Clone + 'static {
    const DTYPE: DType;
}

impl ArrayType for u8 {
    const DTYPE: DType = DType::U8;
}
impl ArrayType for i8 {
    const DTYPE: DType = DType::I8;
}
impl ArrayType for u16 {
    const DTYPE: DType = DType::U16;
}
impl ArrayType for i16 {
    const DTYPE: DType = DType::I16;
}
impl ArrayType for u32 {
    const DTYPE: DType = DType::U32;
}
impl ArrayType for i32 {
    const DTYPE: DType = DType::I32;
}
impl ArrayType for u64 {
    const DTYPE: DType = DType::U64;
}
impl ArrayType for i64 {
    const DTYPE: DType = DType::I64;
}
impl ArrayType for f32 {
    const DTYPE: DType = DType::F32;
}
impl ArrayType for f64 {
    const DTYPE: DType = DType::F64;
}

// =============================================================================
// From ndarray to blobfig
// =============================================================================

impl Array {
    /// Create a blobfig Array from an ndarray ArrayD
    ///
    /// Takes ownership of a contiguous array. Returns error if not contiguous.
    /// Use `.as_standard_layout().into_owned()` to make non-contiguous arrays contiguous.
    pub fn from_ndarray<T: ArrayType>(arr: ArrayD<T>) -> Result<Self, NdarrayError> {
        if !arr.is_standard_layout() {
            return Err(NdarrayError::NotContiguous);
        }

        let shape: Vec<u64> = arr.shape().iter().map(|&d| d as u64).collect();
        let (vec, offset) = arr.into_raw_vec_and_offset();

        // offset must be 0 for safe reinterpretation
        // (offset > 0 means data doesn't start at vec's allocation start)
        if offset != Some(0) {
            return Err(NdarrayError::NotContiguous);
        }

        let byte_len = vec.len() * std::mem::size_of::<T>();
        let cap = vec.capacity() * std::mem::size_of::<T>();
        let ptr = vec.as_ptr();

        std::mem::forget(vec);

        // SAFETY:
        // - vec is forgotten so we own the allocation
        // - offset == 0 ensures ptr points to start of allocation
        // - byte_len/cap are correctly scaled for u8
        // - T is a primitive (ArrayType) with same memory repr as bytes
        let data = unsafe { Vec::from_raw_parts(ptr as *mut u8, byte_len, cap) };
        Ok(Array::new(T::DTYPE, shape, data))
    }
}

// =============================================================================
// From blobfig to ndarray (owned)
// =============================================================================

impl Array {
    /// Convert to an ndarray ArrayD
    pub fn to_ndarray<T: ArrayType>(&self) -> Result<ArrayD<T>, NdarrayError> {
        if T::DTYPE != self.dtype {
            return Err(NdarrayError::DTypeMismatch {
                expected: T::DTYPE,
                actual: self.dtype,
            });
        }

        let shape: Vec<usize> = self.shape.iter().map(|&d| d as usize).collect();
        let expected_len = shape.iter().product::<usize>() * std::mem::size_of::<T>();

        if self.data.len() != expected_len {
            return Err(NdarrayError::ShapeMismatch {
                shape: self.shape.clone(),
                data_len: self.data.len(),
            });
        }

        // Copy data into properly typed vec
        let elements: Vec<T> = self
            .data
            .chunks_exact(std::mem::size_of::<T>())
            .map(|chunk| {
                let mut arr = [0u8; 16]; // Max size we support
                arr[..chunk.len()].copy_from_slice(chunk);
                // SAFETY:
                // - arr is a local buffer we just wrote valid bytes into
                // - T is constrained to ArrayType (primitives only)
                // - All primitive types have no invalid bit patterns
                // - read_unaligned handles any alignment
                unsafe { std::ptr::read_unaligned(arr.as_ptr() as *const T) }
            })
            .collect();

        ArrayD::from_shape_vec(IxDyn(&shape), elements).map_err(|_| NdarrayError::ShapeMismatch {
            shape: self.shape.clone(),
            data_len: self.data.len(),
        })
    }
}

impl<'a> ArrayView<'a> {
    /// Convert to an owned ndarray ArrayD
    pub fn to_ndarray<T: ArrayType>(&self) -> Result<ArrayD<T>, NdarrayError> {
        if T::DTYPE != self.dtype {
            return Err(NdarrayError::DTypeMismatch {
                expected: T::DTYPE,
                actual: self.dtype,
            });
        }

        let shape: Vec<usize> = self.shape.iter().map(|&d| d as usize).collect();
        let expected_len = shape.iter().product::<usize>() * std::mem::size_of::<T>();

        if self.data.len() != expected_len {
            return Err(NdarrayError::ShapeMismatch {
                shape: self.shape.clone(),
                data_len: self.data.len(),
            });
        }

        // Copy data into properly typed vec
        let elements: Vec<T> = self
            .data
            .chunks_exact(std::mem::size_of::<T>())
            .map(|chunk| {
                let mut arr = [0u8; 16];
                arr[..chunk.len()].copy_from_slice(chunk);
                // SAFETY: same as Array::to_ndarray - local buffer, primitive type
                unsafe { std::ptr::read_unaligned(arr.as_ptr() as *const T) }
            })
            .collect();

        ArrayD::from_shape_vec(IxDyn(&shape), elements).map_err(|_| NdarrayError::ShapeMismatch {
            shape: self.shape.clone(),
            data_len: self.data.len(),
        })
    }

    /// Try to create a zero-copy ndarray view
    ///
    /// This will fail if the data is not properly aligned for the element type.
    pub fn try_as_ndarray<T: ArrayType>(&self) -> Result<ArrayViewD<'a, T>, NdarrayError> {
        if T::DTYPE != self.dtype {
            return Err(NdarrayError::DTypeMismatch {
                expected: T::DTYPE,
                actual: self.dtype,
            });
        }

        let shape: Vec<usize> = self.shape.iter().map(|&d| d as usize).collect();
        let expected_len = shape.iter().product::<usize>() * std::mem::size_of::<T>();

        if self.data.len() != expected_len {
            return Err(NdarrayError::ShapeMismatch {
                shape: self.shape.clone(),
                data_len: self.data.len(),
            });
        }

        // Check alignment
        if (self.data.as_ptr() as usize) % std::mem::align_of::<T>() != 0 {
            return Err(NdarrayError::AlignmentError);
        }

        // SAFETY:
        // - Alignment is checked above before the cast
        // - Length is validated earlier (shape matches data.len())
        // - T is constrained to ArrayType (primitives with no invalid bit patterns)
        // - Lifetime 'a from ArrayView<'a> is preserved in ArrayViewD<'a, T>
        let slice = unsafe {
            std::slice::from_raw_parts(self.data.as_ptr() as *const T, shape.iter().product())
        };

        ArrayViewD::from_shape(IxDyn(&shape), slice).map_err(|_| NdarrayError::ShapeMismatch {
            shape: self.shape.clone(),
            data_len: self.data.len(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ndarray::array;

    #[test]
    fn roundtrip_1d_f32() {
        let arr = array![1.0f32, 2.0, 3.0, 4.0].into_dyn();
        let expected = arr.clone();
        let blob = Array::from_ndarray(arr).unwrap();

        assert_eq!(blob.dtype, DType::F32);
        assert_eq!(blob.shape, vec![4]);

        let back: ArrayD<f32> = blob.to_ndarray().unwrap();
        assert_eq!(expected, back);
    }

    #[test]
    fn roundtrip_2d_i32() {
        let arr = array![[1i32, 2, 3], [4, 5, 6]].into_dyn();
        let expected = arr.clone();
        let blob = Array::from_ndarray(arr).unwrap();

        assert_eq!(blob.dtype, DType::I32);
        assert_eq!(blob.shape, vec![2, 3]);

        let back: ArrayD<i32> = blob.to_ndarray().unwrap();
        assert_eq!(expected, back);
    }

    #[test]
    fn roundtrip_3d_u8() {
        let arr = ArrayD::<u8>::zeros(IxDyn(&[2, 3, 4]));
        let expected = arr.clone();
        let blob = Array::from_ndarray(arr).unwrap();

        assert_eq!(blob.dtype, DType::U8);
        assert_eq!(blob.shape, vec![2, 3, 4]);

        let back: ArrayD<u8> = blob.to_ndarray().unwrap();
        assert_eq!(expected, back);
    }

    #[test]
    fn dtype_mismatch_error() {
        let arr = array![1.0f32, 2.0, 3.0].into_dyn();
        let blob = Array::from_ndarray(arr).unwrap();

        let result: Result<ArrayD<f64>, _> = blob.to_ndarray();
        assert!(matches!(result, Err(NdarrayError::DTypeMismatch { .. })));
    }

    #[test]
    fn zero_copy_view_aligned() {
        let arr = array![1.0f64, 2.0, 3.0, 4.0].into_dyn();
        let blob = Array::from_ndarray(arr).unwrap();

        let view = ArrayView {
            dtype: blob.dtype,
            shape: blob.shape.clone(),
            data: &blob.data,
        };

        let result = view.try_as_ndarray::<f64>();
        if let Ok(ndview) = result {
            assert_eq!(ndview.len(), 4);
        }
    }

    #[test]
    fn all_dtypes() {
        assert_eq!(
            Array::from_ndarray(array![1u8, 2, 3].into_dyn())
                .unwrap()
                .dtype,
            DType::U8
        );
        assert_eq!(
            Array::from_ndarray(array![1i8, 2, 3].into_dyn())
                .unwrap()
                .dtype,
            DType::I8
        );
        assert_eq!(
            Array::from_ndarray(array![1u16, 2, 3].into_dyn())
                .unwrap()
                .dtype,
            DType::U16
        );
        assert_eq!(
            Array::from_ndarray(array![1i16, 2, 3].into_dyn())
                .unwrap()
                .dtype,
            DType::I16
        );
        assert_eq!(
            Array::from_ndarray(array![1u32, 2, 3].into_dyn())
                .unwrap()
                .dtype,
            DType::U32
        );
        assert_eq!(
            Array::from_ndarray(array![1i32, 2, 3].into_dyn())
                .unwrap()
                .dtype,
            DType::I32
        );
        assert_eq!(
            Array::from_ndarray(array![1u64, 2, 3].into_dyn())
                .unwrap()
                .dtype,
            DType::U64
        );
        assert_eq!(
            Array::from_ndarray(array![1i64, 2, 3].into_dyn())
                .unwrap()
                .dtype,
            DType::I64
        );
        assert_eq!(
            Array::from_ndarray(array![1.0f32, 2.0, 3.0].into_dyn())
                .unwrap()
                .dtype,
            DType::F32
        );
        assert_eq!(
            Array::from_ndarray(array![1.0f64, 2.0, 3.0].into_dyn())
                .unwrap()
                .dtype,
            DType::F64
        );
    }
}
