//! areamy error integration for blobfig
//!
//! This module provides conversions from blobfig errors to areamy's AnyErr trait.
//!
//! Enable with the `areamy` feature flag.

use crate::error::AccessError;
use areamy::any_err;
use areamy::error::{AnyErr, Error};

impl AnyErr for AccessError {}

impl From<AccessError> for Box<dyn AnyErr> {
    fn from(value: AccessError) -> Self {
        Box::new(value)
    }
}

impl From<AccessError> for Error {
    fn from(value: AccessError) -> Self {
        any_err!(value)
    }
}

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
    use super::*;
    use crate::types::ValueTag;

    #[test]
    fn access_error_not_found_to_anyerr() {
        let err = AccessError::NotFound {
            path: "foo/bar".to_string(),
        };

        let boxed: Box<dyn AnyErr> = err.into();
        assert!(boxed.to_string().contains("foo/bar"));
    }

    #[test]
    fn access_error_type_mismatch_to_anyerr() {
        let err = AccessError::TypeMismatch {
            path: "config/value".to_string(),
            expected: "int",
            actual: ValueTag::String,
        };

        let boxed: Box<dyn AnyErr> = err.into();
        assert!(boxed.to_string().contains("config/value"));
        assert!(boxed.to_string().contains("int"));
    }

    #[cfg(feature = "ndarray")]
    #[test]
    fn ndarray_error_to_anyerr() {
        use crate::ndarray_ext::NdarrayError;
        use crate::types::DType;

        let err = NdarrayError::DTypeMismatch {
            expected: DType::F32,
            actual: DType::F64,
        };

        let boxed: Box<dyn AnyErr> = err.into();
        assert!(boxed.to_string().contains("DType mismatch"));
    }
}
