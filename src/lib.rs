//! blobfig - Binary configuration format with zero-copy parsing
//!
//! A portable binary format for bundling configuration, typed arrays, and file blobs
//! into a single artifact. Designed for ML applications that need to package models,
//! tokenizers, and configuration together.
//!
//! # Features
//!
//! - Zero-copy parsing from memory-mapped files
//! - Streaming write support for large files
//! - Typed arrays (numpy-like) with shape information
//! - Nested key-value structure (like JSON)
//! - Little-endian, portable across architectures
//!
//! # Example
//!
//! ```rust
//! use blobfig::{Value, File, Array, DType, writer};
//!
//! // Build a config
//! let config = Value::Object(vec![
//!     ("version".into(), Value::Int(1)),
//!     ("model".into(), Value::File(
//!         File::from_bytes("application/x-tflite", vec![/* model bytes */])
//!     )),
//!     ("stats".into(), Value::Object(vec![
//!         ("mean".into(), Value::Array(
//!             Array::new(DType::F32, vec![80], vec![0u8; 320])
//!         )),
//!     ])),
//! ]);
//!
//! // Write to bytes
//! let bytes = writer::to_bytes(config).unwrap();
//! ```

pub mod error;
pub mod parser;
pub mod types;
pub mod writer;

#[cfg(feature = "ndarray")]
pub mod ndarray_ext;

#[cfg(feature = "areamy")]
pub mod areamy_ext;

// Re-export common types at crate root
pub use parser::parse;
pub use types::{
    Array, ArrayView, DType, File, FileData, FileHandle, FileView, HEADER_SIZE, MAGIC, VERSION,
    Value, ValueTag, ValueView,
};

#[cfg(feature = "ndarray")]
pub use ndarray_ext::{ArrayType, NdarrayError};
