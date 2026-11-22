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

/// Trait for types that can be stored in a blobfig array: the ten numeric
/// types of [`DType`]
///
/// Sealed: converting copies raw bytes into these types, which is only sound
/// for plain numbers, where any bytes are a valid value.
pub trait ArrayType: sealed::Sealed + Sized + Clone + 'static {
    const DTYPE: DType;
}

mod sealed {
    pub trait Sealed {}
    impl Sealed for u8 {}
    impl Sealed for i8 {}
    impl Sealed for u16 {}
    impl Sealed for i16 {}
    impl Sealed for u32 {}
    impl Sealed for i32 {}
    impl Sealed for u64 {}
    impl Sealed for i64 {}
    impl Sealed for f32 {}
    impl Sealed for f64 {}
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

/// The array's shape as `usize`s for ndarray, after checking that `T` is its
/// dtype and that `data` is exactly the size the dtype and shape need
fn checked_shape<T: ArrayType>(
    dtype: DType,
    shape: &[u64],
    data: &[u8],
) -> Result<Vec<usize>, NdarrayError> {
    if T::DTYPE != dtype {
        return Err(NdarrayError::DTypeMismatch {
            expected: T::DTYPE,
            actual: dtype,
        });
    }
    let shape_mismatch = || NdarrayError::ShapeMismatch {
        shape: shape.to_vec(),
        data_len: data.len(),
    };
    if dtype.data_size(shape) != Some(data.len() as u64) {
        return Err(shape_mismatch());
    }
    shape
        .iter()
        .map(|&dim| usize::try_from(dim).map_err(|_| shape_mismatch()))
        .collect()
}

/// An owned ndarray of the array with `dtype`, `shape` and `data`, copying
/// `data` into it in one go
fn copy_to_ndarray<T: ArrayType>(
    dtype: DType,
    shape: &[u64],
    data: &[u8],
) -> Result<ArrayD<T>, NdarrayError> {
    let dims = checked_shape::<T>(dtype, shape, data)?;
    let len = data.len() / std::mem::size_of::<T>();
    let bytes = len * std::mem::size_of::<T>();
    let mut elements = Vec::<T>::with_capacity(len);
    // SAFETY: `bytes` is at most `data.len()` and exactly the room `elements`
    // has for `len` values, and the two don't overlap, so the copy stays in
    // bounds whatever the checks above found. `T` is one of the plain numeric
    // types `ArrayType` is sealed to, for which any bytes are a valid value,
    // so after the copy the first `len` values are initialized.
    unsafe {
        std::ptr::copy_nonoverlapping(data.as_ptr(), elements.as_mut_ptr().cast(), bytes);
        elements.set_len(len);
    }
    ArrayD::from_shape_vec(IxDyn(&dims), elements).map_err(|_| NdarrayError::ShapeMismatch {
        shape: shape.to_vec(),
        data_len: data.len(),
    })
}

impl Array {
    /// Convert to an ndarray ArrayD
    pub fn to_ndarray<T: ArrayType>(&self) -> Result<ArrayD<T>, NdarrayError> {
        copy_to_ndarray(self.dtype, &self.shape, &self.data)
    }
}

impl<'a> ArrayView<'a> {
    /// Convert to an owned ndarray ArrayD (a copy; see
    /// [`try_as_ndarray`](Self::try_as_ndarray) for a view)
    pub fn to_ndarray<T: ArrayType>(&self) -> Result<ArrayD<T>, NdarrayError> {
        copy_to_ndarray(self.dtype, &self.shape, self.data)
    }

    /// Try to create a zero-copy ndarray view
    ///
    /// This will fail if the data is not properly aligned for the element type.
    pub fn try_as_ndarray<T: ArrayType>(&self) -> Result<ArrayViewD<'a, T>, NdarrayError> {
        let dims = checked_shape::<T>(self.dtype, &self.shape, self.data)?;
        if !(self.data.as_ptr() as usize).is_multiple_of(std::mem::align_of::<T>()) {
            return Err(NdarrayError::AlignmentError);
        }
        // SAFETY: the data is aligned for `T` (checked above), and the slice
        // covers `data.len() / size_of::<T>()` values, which lie within
        // `data`. `T` is one of the plain numeric types `ArrayType` is sealed
        // to, for which any bytes are a valid value. The slice borrows `data`
        // for `'a`, as the returned view does.
        let len = self.data.len() / std::mem::size_of::<T>();
        let slice = unsafe { std::slice::from_raw_parts(self.data.as_ptr().cast::<T>(), len) };
        ArrayViewD::from_shape(IxDyn(&dims), slice).map_err(|_| NdarrayError::ShapeMismatch {
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
    fn size_mismatch_error() {
        // One f32 is 4 bytes: a fifth isn't silently dropped
        let array = Array::new(DType::F32, vec![1], vec![0; 5]);
        let error = array.to_ndarray::<f32>().unwrap_err();
        assert!(matches!(
            error,
            NdarrayError::ShapeMismatch { data_len: 5, .. }
        ));
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
