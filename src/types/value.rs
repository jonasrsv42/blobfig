//! Owned value type and the value tag shared with its borrowed counterpart.
//!
//! The borrowed [`ValueView`](super::ValueView) lives in `view.rs`; the read
//! accessors common to both are on the [`NodeView`](super::NodeView) trait in
//! `node_view.rs`.

use super::{Array, File, NodeView, ValueTag};

/// What a secret value renders as when printed. Single source of truth so the
/// `Value` and `ValueView` `Debug`/`Display` impls cannot drift apart.
pub(crate) const REDACTED: &str = "<redacted>";

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

    /// Object entries as a slice (sees through a secret wrapper). The borrowed
    /// counterpart's keys are `&str`; here they are owned `String`s, so this
    /// stays inherent rather than living on [`NodeView`].
    pub fn as_object(&self) -> Option<&[(String, Value)]> {
        match self.peel_secret() {
            Value::Object(o) => Some(o),
            _ => None,
        }
    }
}

impl NodeView for Value {
    type Array = Array;
    type File = File;

    fn tag(&self) -> ValueTag {
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

    fn peel_secret(&self) -> &Self {
        let mut current = self;
        while let Value::Secret(inner) = current {
            current = inner;
        }
        current
    }

    fn as_bool(&self) -> Option<bool> {
        match self.peel_secret() {
            Value::Bool(b) => Some(*b),
            _ => None,
        }
    }

    fn as_int(&self) -> Option<i64> {
        match self.peel_secret() {
            Value::Int(i) => Some(*i),
            _ => None,
        }
    }

    fn as_float(&self) -> Option<f64> {
        match self.peel_secret() {
            Value::Float(f) => Some(*f),
            _ => None,
        }
    }

    fn as_str(&self) -> Option<&str> {
        match self.peel_secret() {
            Value::String(s) => Some(s),
            _ => None,
        }
    }

    fn as_array(&self) -> Option<&Array> {
        match self.peel_secret() {
            Value::Array(a) => Some(a),
            _ => None,
        }
    }

    fn as_file(&self) -> Option<&File> {
        match self.peel_secret() {
            Value::File(f) => Some(f),
            _ => None,
        }
    }

    fn as_list(&self) -> Option<&[Self]> {
        match self.peel_secret() {
            Value::List(l) => Some(l),
            _ => None,
        }
    }

    fn object_entries(&self) -> Option<impl Iterator<Item = (&str, &Self)>> {
        match self {
            Value::Object(entries) => Some(entries.iter().map(|(k, v)| (k.as_str(), v))),
            _ => None,
        }
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

    #[test]
    fn owned_value_path_accessors() {
        // The path API now works directly on an owned `Value`, no round-trip.
        let v = Value::Object(vec![(
            "audio".into(),
            Value::Object(vec![(
                "speaker".into(),
                Value::Object(vec![("volume".into(), Value::Float(0.8))]),
            )]),
        )]);
        assert_eq!(v.float("audio/speaker/volume").unwrap(), 0.8);
        assert!(v.get("audio/speaker/missing").is_none());
        assert!(v.string("audio/speaker/volume").is_err());
    }

    #[test]
    fn owned_value_path_sees_through_secret() {
        let v = Value::Object(vec![(
            "db".into(),
            Value::secret(Value::Object(vec![(
                "password".into(),
                Value::String("s3cr3t".into()),
            )])),
        )]);
        assert_eq!(v.string("db/password").unwrap(), "s3cr3t");
    }
}
