//! areamy error integration for blobfig
//!
//! This module provides conversions from blobfig errors to areamy's AnyErr trait.
//!
//! Enable with the `areamy` feature flag.

// NdarrayError integration (when both areamy and ndarray features are enabled)
#[cfg(feature = "ndarray")]
mod ndarray_errors {
    use crate::ndarray_ext::NdarrayError;
    use areamy::error::AnyErr;

    impl AnyErr for NdarrayError {}

    impl From<NdarrayError> for Box<dyn AnyErr> {
        fn from(value: NdarrayError) -> Self {
            Box::new(value)
        }
    }
}

#[cfg(test)]
mod tests {
    #[cfg(feature = "ndarray")]
    #[test]
    fn ndarray_error_to_anyerr() {
        use crate::ndarray_ext::NdarrayError;
        use crate::types::DType;
        use areamy::error::AnyErr;

        let err = NdarrayError::DTypeMismatch {
            expected: DType::F32,
            actual: DType::F64,
        };

        let boxed: Box<dyn AnyErr> = err.into();
        assert!(boxed.to_string().contains("DType mismatch"));
    }
}
