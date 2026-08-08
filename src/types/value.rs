//! Owned value type and the value tag shared with its borrowed counterpart.
//!
//! The borrowed [`ValueView`](super::ValueView) lives in `view.rs`; the read
//! accessors common to both are on the [`NodeView`](super::NodeView) trait in
//! `node_view.rs`.

use super::{Array, File, List, NodeView, Object, ValueTag};

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
    Object(Object),
    List(List),
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

    /// An object value from its `(key, value)` entries — shorthand for
    /// `Value::Object(Object::new(entries))`.
    pub fn object(entries: Vec<(String, Value)>) -> Value {
        Value::Object(Object::new(entries))
    }

    /// A list value from its items — shorthand for
    /// `Value::List(List::new(items))`.
    pub fn list(items: Vec<Value>) -> Value {
        Value::List(List::new(items))
    }
}

impl NodeView for Value {
    type Array = Array;
    type File = File;
    type Object = Object;
    type List = List;

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

    fn as_object(&self) -> Option<&Object> {
        match self.peel_secret() {
            Value::Object(o) => Some(o),
            _ => None,
        }
    }

    fn as_list(&self) -> Option<&List> {
        match self.peel_secret() {
            Value::List(l) => Some(l),
            _ => None,
        }
    }
}

impl TryFrom<Value> for Object {
    type Error = Value;

    /// Detach an owned object, moving it out — seeing through `Secret` wrappers
    /// like every other typed accessor. A value that isn't an object is handed
    /// back **with its secret wrapper intact**, so a failed detach never strips
    /// the redaction guard from a value the caller still holds.
    fn try_from(value: Value) -> Result<Self, Self::Error> {
        match value {
            Value::Object(object) => Ok(object),
            Value::Secret(inner) => {
                Object::try_from(*inner).map_err(|inner| Value::Secret(Box::new(inner)))
            }
            other => Err(other),
        }
    }
}

impl TryFrom<Value> for List {
    type Error = Value;

    /// Detach an owned list, moving it out — seeing through `Secret` wrappers
    /// like every other typed accessor. A value that isn't a list is handed back
    /// **with its secret wrapper intact**, so a failed detach never strips the
    /// redaction guard from a value the caller still holds.
    fn try_from(value: Value) -> Result<Self, Self::Error> {
        match value {
            Value::List(list) => Ok(list),
            Value::Secret(inner) => {
                List::try_from(*inner).map_err(|inner| Value::Secret(Box::new(inner)))
            }
            other => Err(other),
        }
    }
}

impl TryFrom<Value> for String {
    type Error = Value;

    /// Move a `String` out — the owning counterpart to
    /// [`as_str`](NodeView::as_str), seeing through `Secret` wrappers. A value
    /// that isn't a string is handed back with its secret wrapper intact.
    fn try_from(value: Value) -> Result<Self, Self::Error> {
        match value {
            Value::String(string) => Ok(string),
            Value::Secret(inner) => {
                String::try_from(*inner).map_err(|inner| Value::Secret(Box::new(inner)))
            }
            other => Err(other),
        }
    }
}

impl TryFrom<Value> for Array {
    type Error = Value;

    /// Move an [`Array`] out — the owning counterpart to
    /// [`as_array`](NodeView::as_array), seeing through `Secret` wrappers. A
    /// value that isn't an array is handed back with its secret wrapper intact.
    fn try_from(value: Value) -> Result<Self, Self::Error> {
        match value {
            Value::Array(array) => Ok(array),
            Value::Secret(inner) => {
                Array::try_from(*inner).map_err(|inner| Value::Secret(Box::new(inner)))
            }
            other => Err(other),
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
        let v = Value::object(vec![
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
        let v = Value::secret(Value::object(vec![
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
        // The path API works directly on an owned `Value`, no round-trip.
        let v = Value::object(vec![(
            "audio".into(),
            Value::object(vec![(
                "speaker".into(),
                Value::object(vec![("volume".into(), Value::Float(0.8))]),
            )]),
        )]);
        assert_eq!(v.float("audio/speaker/volume").unwrap(), 0.8);
        assert!(v.get("audio/speaker/missing").is_none());
        assert!(v.string("audio/speaker/volume").is_err());
    }

    #[test]
    fn owned_value_path_sees_through_secret() {
        let v = Value::object(vec![(
            "db".into(),
            Value::secret(Value::object(vec![(
                "password".into(),
                Value::String("s3cr3t".into()),
            )])),
        )]);
        assert_eq!(v.string("db/password").unwrap(), "s3cr3t");
    }

    #[test]
    fn a_secret_wrapped_object_detaches_through_the_secret() {
        // Consuming detach sees through `Secret`, like every borrowing accessor.
        let value = Value::secret(Value::object(vec![("k".to_owned(), Value::Int(1))]));
        let object = Object::try_from(value).expect("detaches through the secret");
        assert_eq!(object.len(), 1);
    }

    #[test]
    fn a_failed_detach_returns_the_node_with_its_secret_intact() {
        // A secret-wrapped non-object, tried as an object, comes back unchanged
        // — still redacting, so a failed detach never strips the guard.
        let value = Value::secret("top-secret");
        let Err(returned) = Object::try_from(value) else {
            panic!("a string is not an object");
        };
        assert_eq!(format!("{returned:?}"), "<redacted>");
        assert_eq!(returned.as_str(), Some("top-secret"));
    }

    #[test]
    fn try_object_then_list_classifies_a_secret_container() {
        // The canonical consuming shape: try object, else list, else leaf. A
        // secret-wrapped container classifies by its inner type, not as a leaf.
        fn shape<Node: NodeView>(node: Node) -> &'static str {
            match TryInto::<Node::Object>::try_into(node) {
                Ok(_) => "object",
                Err(node) => match TryInto::<Node::List>::try_into(node) {
                    Ok(_) => "list",
                    Err(_) => "leaf",
                },
            }
        }
        assert_eq!(
            shape(Value::secret(Value::list(vec![Value::Int(1)]))),
            "list",
        );
        assert_eq!(shape(Value::Int(7)), "leaf");
    }

    #[test]
    fn a_string_moves_out_through_its_secret() {
        let value = Value::secret("token");
        let string = String::try_from(value).expect("moves the string out");
        assert_eq!(string, "token");
    }

    #[test]
    fn a_non_string_is_handed_back_with_its_secret_intact() {
        let value = Value::secret(Value::Int(7));
        let Err(returned) = String::try_from(value) else {
            panic!("an int is not a string");
        };
        assert_eq!(format!("{returned:?}"), "<redacted>");
        assert_eq!(returned.as_int(), Some(7));
    }

    #[test]
    fn an_array_moves_out() {
        use crate::DType;
        let value = Value::Array(Array::new(DType::U8, vec![3], vec![1, 2, 3]));
        let array = Array::try_from(value).expect("moves the array out");
        assert_eq!(array.data, vec![1, 2, 3]);
    }
}
