//! Rules that both the parser and the writer enforce, so a blob the writer
//! produces always parses, and both report a broken rule the same way

mod data_size;
mod depth;
mod keys;

pub(crate) use data_size::{DataSizeError, check_data_size};
pub use depth::MAX_DEPTH;
pub(crate) use keys::{KeyProblem, check_keys};
