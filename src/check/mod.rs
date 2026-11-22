//! Rules that both the parser and the writer enforce, so a blob the writer
//! produces always parses, and both report a broken rule the same way

mod data_size;
mod keys;

pub(crate) use data_size::check_data_size;
pub(crate) use keys::check_keys;
