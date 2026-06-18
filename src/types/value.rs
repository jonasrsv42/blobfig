//! Value types - owned and view variants

use super::{Array, ArrayView, File, FileView};
use crate::error::AccessError;

/// What a secret value renders as when printed. Single source of truth so the
/// `Value` and `ValueView` `Debug` impls cannot drift apart.
pub(crate) const REDACTED: &str = "<redacted>";

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
    /// Wraps another value, marking it secret: redacted when printed.
    Secret = 0x09,
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
            0x09 => Some(ValueTag::Secret),
            _ => None,
        }
    }
}

/// Owned blobfig value (for building/writing)
pub enum Value {
    Bool(bool),
    Int(i64),
    Float(f64),
    String(String),
    Array(Array),
    File(File),
    Object(Vec<(String, Value)>),
    List(Vec<Value>),
    /// A value marked secret: prints as `<redacted>`, but is otherwise a normal
    /// value (stored in plaintext on the wire — this is redaction, not encryption).
    Secret(Box<Value>),
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
            Value::Secret(_) => ValueTag::Secret,
        }
    }

    /// Mark a value as secret so it is redacted when printed.
    ///
    /// ```
    /// use blobfig::Value;
    /// let pw = Value::secret("hunter2");
    /// assert_eq!(format!("{:?}", pw), "<redacted>");
    /// ```
    pub fn secret(value: impl Into<Value>) -> Value {
        Value::Secret(Box::new(value.into()))
    }
}

// Hand-written so the `Secret` arm never formats its inner value at any depth.
// All other arms mirror what `#[derive(Debug)]` would produce.
impl std::fmt::Debug for Value {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Value::Secret(_) => f.write_str(REDACTED),
            Value::Bool(b) => f.debug_tuple("Bool").field(b).finish(),
            Value::Int(i) => f.debug_tuple("Int").field(i).finish(),
            Value::Float(x) => f.debug_tuple("Float").field(x).finish(),
            Value::String(s) => f.debug_tuple("String").field(s).finish(),
            Value::Array(a) => f.debug_tuple("Array").field(a).finish(),
            Value::File(file) => f.debug_tuple("File").field(file).finish(),
            Value::Object(entries) => f.debug_tuple("Object").field(entries).finish(),
            Value::List(items) => f.debug_tuple("List").field(items).finish(),
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
#[derive(Clone)]
pub enum ValueView<'a> {
    Bool(bool),
    Int(i64),
    Float(f64),
    String(&'a str),
    Array(ArrayView<'a>),
    File(FileView<'a>),
    Object(Vec<(&'a str, ValueView<'a>)>),
    List(Vec<ValueView<'a>>),
    /// A value marked secret: prints as `<redacted>`. Programmatic access
    /// (`get` and the typed accessors) sees through it transparently — only
    /// printing is redacted. The bytes are stored in plaintext, not encrypted.
    Secret(Box<ValueView<'a>>),
}

// Hand-written so the `Secret` arm never formats its inner value at any depth.
// All other arms mirror what `#[derive(Debug)]` would produce.
impl<'a> std::fmt::Debug for ValueView<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ValueView::Secret(_) => f.write_str(REDACTED),
            ValueView::Bool(b) => f.debug_tuple("Bool").field(b).finish(),
            ValueView::Int(i) => f.debug_tuple("Int").field(i).finish(),
            ValueView::Float(x) => f.debug_tuple("Float").field(x).finish(),
            ValueView::String(s) => f.debug_tuple("String").field(s).finish(),
            ValueView::Array(a) => f.debug_tuple("Array").field(a).finish(),
            ValueView::File(file) => f.debug_tuple("File").field(file).finish(),
            ValueView::Object(entries) => f.debug_tuple("Object").field(entries).finish(),
            ValueView::List(items) => f.debug_tuple("List").field(items).finish(),
        }
    }
}

impl<'a> ValueView<'a> {
    /// Peel any `Secret` wrappers to the inner value (returns `self` when not
    /// secret). Used by the accessors so that explicit, typed access sees
    /// through secrets — only `Debug` printing is redacted.
    fn peel_secret(&self) -> &ValueView<'a> {
        let mut current = self;
        while let ValueView::Secret(inner) = current {
            current = inner;
        }
        current
    }

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
            // `(**inner)` reaches the inner `ValueView` so this calls the
            // inherent `to_owned`, not `ToOwned::to_owned` on the `Box`.
            ValueView::Secret(inner) => Value::Secret(Box::new((**inner).to_owned())),
        }
    }

    /// Try to get as bool (sees through a secret wrapper)
    pub fn as_bool(&self) -> Option<bool> {
        match self.peel_secret() {
            ValueView::Bool(b) => Some(*b),
            _ => None,
        }
    }

    /// Try to get as i64 (sees through a secret wrapper)
    pub fn as_int(&self) -> Option<i64> {
        match self.peel_secret() {
            ValueView::Int(i) => Some(*i),
            _ => None,
        }
    }

    /// Try to get as f64 (sees through a secret wrapper)
    pub fn as_float(&self) -> Option<f64> {
        match self.peel_secret() {
            ValueView::Float(f) => Some(*f),
            _ => None,
        }
    }

    /// Try to get as string (sees through a secret wrapper)
    pub fn as_str(&self) -> Option<&'a str> {
        match self.peel_secret() {
            ValueView::String(s) => Some(s),
            _ => None,
        }
    }

    /// Try to get as array (sees through a secret wrapper)
    pub fn as_array(&self) -> Option<&ArrayView<'a>> {
        match self.peel_secret() {
            ValueView::Array(a) => Some(a),
            _ => None,
        }
    }

    /// Try to get as file (sees through a secret wrapper)
    pub fn as_file(&self) -> Option<&FileView<'a>> {
        match self.peel_secret() {
            ValueView::File(f) => Some(f),
            _ => None,
        }
    }

    /// Try to get as object (sees through a secret wrapper)
    pub fn as_object(&self) -> Option<&[(&'a str, ValueView<'a>)]> {
        match self.peel_secret() {
            ValueView::Object(o) => Some(o),
            _ => None,
        }
    }

    /// Try to get as list (sees through a secret wrapper)
    pub fn as_list(&self) -> Option<&[ValueView<'a>]> {
        match self.peel_secret() {
            ValueView::List(l) => Some(l),
            _ => None,
        }
    }

    /// Get a nested value by path (e.g., "audio/sample_rate").
    ///
    /// Traversal sees through `Secret` wrappers, so a secret sub-object can be
    /// descended into (`get("db/password")` works). The returned node is left
    /// as-is, so a secret leaf comes back wrapped and still redacts when printed.
    pub fn get(&self, path: &str) -> Option<&ValueView<'a>> {
        let mut current = self;
        for key in path.split('/') {
            match current.peel_secret() {
                ValueView::Object(entries) => {
                    current = entries.iter().find(|(k, _)| *k == key).map(|(_, v)| v)?;
                }
                _ => return None,
            }
        }
        Some(current)
    }

    // =========================================================================
    // Convenience accessors that return Result with path context
    // =========================================================================

    /// Get a bool at path
    pub fn bool(&self, path: &str) -> Result<bool, AccessError> {
        let value = self.get(path).ok_or_else(|| AccessError::NotFound {
            path: path.to_string(),
        })?;
        value.as_bool().ok_or_else(|| AccessError::TypeMismatch {
            path: path.to_string(),
            expected: "bool",
            actual: value.peel_secret().tag(),
        })
    }

    /// Get an i64 at path
    pub fn int(&self, path: &str) -> Result<i64, AccessError> {
        let value = self.get(path).ok_or_else(|| AccessError::NotFound {
            path: path.to_string(),
        })?;
        value.as_int().ok_or_else(|| AccessError::TypeMismatch {
            path: path.to_string(),
            expected: "int",
            actual: value.peel_secret().tag(),
        })
    }

    /// Get an f64 at path
    pub fn float(&self, path: &str) -> Result<f64, AccessError> {
        let value = self.get(path).ok_or_else(|| AccessError::NotFound {
            path: path.to_string(),
        })?;
        value.as_float().ok_or_else(|| AccessError::TypeMismatch {
            path: path.to_string(),
            expected: "float",
            actual: value.peel_secret().tag(),
        })
    }

    /// Get a string at path
    pub fn string(&self, path: &str) -> Result<&'a str, AccessError> {
        let value = self.get(path).ok_or_else(|| AccessError::NotFound {
            path: path.to_string(),
        })?;
        value.as_str().ok_or_else(|| AccessError::TypeMismatch {
            path: path.to_string(),
            expected: "string",
            actual: value.peel_secret().tag(),
        })
    }

    /// Get an array at path
    pub fn array(&self, path: &str) -> Result<&ArrayView<'a>, AccessError> {
        let value = self.get(path).ok_or_else(|| AccessError::NotFound {
            path: path.to_string(),
        })?;
        value.as_array().ok_or_else(|| AccessError::TypeMismatch {
            path: path.to_string(),
            expected: "array",
            actual: value.peel_secret().tag(),
        })
    }

    /// Get a file at path
    pub fn file(&self, path: &str) -> Result<&FileView<'a>, AccessError> {
        let value = self.get(path).ok_or_else(|| AccessError::NotFound {
            path: path.to_string(),
        })?;
        value.as_file().ok_or_else(|| AccessError::TypeMismatch {
            path: path.to_string(),
            expected: "file",
            actual: value.peel_secret().tag(),
        })
    }

    /// Get the tag for this value
    pub fn tag(&self) -> ValueTag {
        match self {
            ValueView::Bool(_) => ValueTag::Bool,
            ValueView::Int(_) => ValueTag::Int,
            ValueView::Float(_) => ValueTag::Float,
            ValueView::String(_) => ValueTag::String,
            ValueView::Array(_) => ValueTag::Array,
            ValueView::File(_) => ValueTag::File,
            ValueView::Object(_) => ValueTag::Object,
            ValueView::List(_) => ValueTag::List,
            ValueView::Secret(_) => ValueTag::Secret,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn value_secret_redacts_in_debug() {
        assert_eq!(format!("{:?}", Value::secret("hunter2")), "<redacted>");
        assert_eq!(format!("{:?}", Value::secret(Value::Int(42))), "<redacted>");
    }

    #[test]
    fn value_secret_only_redacts_the_secret() {
        let v = Value::Object(vec![
            ("user".into(), Value::String("visible".into())),
            ("password".into(), Value::secret("hidden")),
        ]);
        let dbg = format!("{:?}", v);
        assert!(dbg.contains("visible"), "{dbg}");
        assert!(!dbg.contains("hidden"), "secret leaked: {dbg}");
        assert!(dbg.contains("<redacted>"), "{dbg}");
    }

    #[test]
    fn value_secret_pretty_debug_also_redacts() {
        let v = Value::secret("hunter2");
        assert!(!format!("{:#?}", v).contains("hunter2"));
    }

    #[test]
    fn nested_secret_never_recurses() {
        // A secret wrapping a whole object must not format any inner field.
        let v = Value::secret(Value::Object(vec![
            ("host".into(), Value::String("db.internal".into())),
            ("pw".into(), Value::String("s3cr3t".into())),
        ]));
        let dbg = format!("{:#?}", v);
        assert_eq!(dbg, "<redacted>");
        assert!(!dbg.contains("db.internal") && !dbg.contains("s3cr3t"));
    }

    #[test]
    fn double_secret_redacts() {
        let v = Value::Secret(Box::new(Value::secret("x")));
        assert_eq!(format!("{:?}", v), "<redacted>");
    }

    #[test]
    fn value_secret_tag() {
        assert_eq!(Value::secret("x").tag(), ValueTag::Secret);
    }
}
