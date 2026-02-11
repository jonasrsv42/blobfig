//! Value types - owned and view variants

use super::{Array, ArrayView, File, FileView};

/// Value type tags
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum ValueTag {
    Bool = 0x01,
    Int = 0x02,
    Float = 0x03,
    String = 0x04,
    Array = 0x05,
    File = 0x06,
    Object = 0x07,
    List = 0x08,
}

impl ValueTag {
    pub fn from_u8(tag: u8) -> Option<Self> {
        match tag {
            0x01 => Some(ValueTag::Bool),
            0x02 => Some(ValueTag::Int),
            0x03 => Some(ValueTag::Float),
            0x04 => Some(ValueTag::String),
            0x05 => Some(ValueTag::Array),
            0x06 => Some(ValueTag::File),
            0x07 => Some(ValueTag::Object),
            0x08 => Some(ValueTag::List),
            _ => None,
        }
    }
}

/// Owned blobfig value (for building/writing)
#[derive(Debug)]
pub enum Value {
    Bool(bool),
    Int(i64),
    Float(f64),
    String(String),
    Array(Array),
    File(File),
    Object(Vec<(String, Value)>),
    List(Vec<Value>),
}

impl Value {
    /// Get the tag for this value
    pub fn tag(&self) -> ValueTag {
        match self {
            Value::Bool(_) => ValueTag::Bool,
            Value::Int(_) => ValueTag::Int,
            Value::Float(_) => ValueTag::Float,
            Value::String(_) => ValueTag::String,
            Value::Array(_) => ValueTag::Array,
            Value::File(_) => ValueTag::File,
            Value::Object(_) => ValueTag::Object,
            Value::List(_) => ValueTag::List,
        }
    }
}

// Convenience From impls for Value
impl From<bool> for Value {
    fn from(v: bool) -> Self {
        Value::Bool(v)
    }
}

impl From<i64> for Value {
    fn from(v: i64) -> Self {
        Value::Int(v)
    }
}

impl From<f64> for Value {
    fn from(v: f64) -> Self {
        Value::Float(v)
    }
}

impl From<String> for Value {
    fn from(v: String) -> Self {
        Value::String(v)
    }
}

impl From<&str> for Value {
    fn from(v: &str) -> Self {
        Value::String(v.to_string())
    }
}

impl From<Array> for Value {
    fn from(v: Array) -> Self {
        Value::Array(v)
    }
}

impl From<File> for Value {
    fn from(v: File) -> Self {
        Value::File(v)
    }
}

/// Parsed blobfig value - references data in the underlying buffer (zero-copy)
#[derive(Debug, Clone)]
pub enum ValueView<'a> {
    Bool(bool),
    Int(i64),
    Float(f64),
    String(&'a str),
    Array(ArrayView<'a>),
    File(FileView<'a>),
    Object(Vec<(&'a str, ValueView<'a>)>),
    List(Vec<ValueView<'a>>),
}

impl<'a> ValueView<'a> {
    /// Convert to owned Value
    pub fn to_owned(&self) -> Value {
        match self {
            ValueView::Bool(b) => Value::Bool(*b),
            ValueView::Int(i) => Value::Int(*i),
            ValueView::Float(f) => Value::Float(*f),
            ValueView::String(s) => Value::String((*s).to_string()),
            ValueView::Array(a) => Value::Array(a.to_owned()),
            ValueView::File(f) => Value::File(f.to_owned()),
            ValueView::Object(entries) => Value::Object(
                entries
                    .iter()
                    .map(|(k, v)| ((*k).to_string(), v.to_owned()))
                    .collect(),
            ),
            ValueView::List(items) => Value::List(items.iter().map(|v| v.to_owned()).collect()),
        }
    }

    /// Try to get as bool
    pub fn as_bool(&self) -> Option<bool> {
        match self {
            ValueView::Bool(b) => Some(*b),
            _ => None,
        }
    }

    /// Try to get as i64
    pub fn as_int(&self) -> Option<i64> {
        match self {
            ValueView::Int(i) => Some(*i),
            _ => None,
        }
    }

    /// Try to get as f64
    pub fn as_float(&self) -> Option<f64> {
        match self {
            ValueView::Float(f) => Some(*f),
            _ => None,
        }
    }

    /// Try to get as string
    pub fn as_str(&self) -> Option<&'a str> {
        match self {
            ValueView::String(s) => Some(s),
            _ => None,
        }
    }

    /// Try to get as array
    pub fn as_array(&self) -> Option<&ArrayView<'a>> {
        match self {
            ValueView::Array(a) => Some(a),
            _ => None,
        }
    }

    /// Try to get as file
    pub fn as_file(&self) -> Option<&FileView<'a>> {
        match self {
            ValueView::File(f) => Some(f),
            _ => None,
        }
    }

    /// Try to get as object
    pub fn as_object(&self) -> Option<&[(&'a str, ValueView<'a>)]> {
        match self {
            ValueView::Object(o) => Some(o),
            _ => None,
        }
    }

    /// Try to get as list
    pub fn as_list(&self) -> Option<&[ValueView<'a>]> {
        match self {
            ValueView::List(l) => Some(l),
            _ => None,
        }
    }

    /// Get a nested value by dot-separated path (e.g., "audio.sample_rate")
    pub fn get(&self, path: &str) -> Option<&ValueView<'a>> {
        let mut current = self;
        for key in path.split('.') {
            match current {
                ValueView::Object(entries) => {
                    current = entries.iter().find(|(k, _)| *k == key).map(|(_, v)| v)?;
                }
                _ => return None,
            }
        }
        Some(current)
    }
}
