//! Borrowed value type — a zero-copy view over data in the underlying buffer.
//!
//! The owned [`Value`](super::Value) lives in `value.rs`; the read accessors
//! common to both are on the [`NodeView`](super::NodeView) trait in `node_view.rs`.
//! `ValueView` additionally keeps inherent `as_str`/`string` that return
//! `&'a str` (decoupled from the `&self` borrow) — its zero-copy superpower,
//! which the trait's `&self`-bound signatures cannot express.

use super::node_view::{ListNode, ObjectNode};
use super::value::REDACTED;
use super::{ArrayView, FileView, List, ListView, NodeView, Object, ObjectView, Value, ValueTag};
use crate::error::AccessError;

/// Parsed blobfig value - references data in the underlying buffer (zero-copy)
#[derive(Clone)]
pub enum ValueView<'a> {
    Bool(bool),
    Int(i64),
    Float(f64),
    String(&'a str),
    Array(ArrayView<'a>),
    File(FileView<'a>),
    Object(ObjectView<'a>),
    List(ListView<'a>),
    /// A value marked secret: prints as `<redacted>`. Programmatic access
    /// (`get` and the typed accessors) sees through it transparently — only
    /// printing is redacted. The bytes are stored in plaintext, not encrypted.
    Secret(Box<ValueView<'a>>),
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
            ValueView::Object(object) => Value::Object(Object::new(
                object
                    .entries()
                    .map(|(k, v)| (k.to_string(), v.to_owned()))
                    .collect(),
            )),
            ValueView::List(list) => Value::List(List::new(
                list.items().iter().map(|v| v.to_owned()).collect(),
            )),
            // `(**inner)` reaches the inner `ValueView` so this calls the
            // inherent `to_owned`, not `ToOwned::to_owned` on the `Box`.
            ValueView::Secret(inner) => Value::Secret(Box::new((**inner).to_owned())),
        }
    }

    /// Try to get as string, returning a borrow of the **underlying buffer**
    /// (`&'a str`), not one tied to `&self`. This decoupling is `ValueView`'s
    /// zero-copy superpower; it shadows the `&self`-bound [`NodeView::as_str`].
    pub fn as_str(&self) -> Option<&'a str> {
        match self.peel_secret() {
            ValueView::String(s) => Some(s),
            _ => None,
        }
    }

    /// Get a string at path, returning a `&'a str` borrow of the underlying
    /// buffer (see [`as_str`](ValueView::as_str)). Shadows the `&self`-bound
    /// [`NodeView::string`].
    pub fn string(&self, path: &str) -> Result<&'a str, AccessError> {
        let value = self.require(path)?;
        value.as_str().ok_or_else(|| value.mismatch(path, "string"))
    }
}

/// Materialize a borrowed view into an owned [`Value`] by cloning out of the
/// source buffer. Paired with the reflexive `Value: Into<Value>` (a move), this
/// lets generic code take `impl Into<Value>` and get the cheapest owned form of
/// either representation — a clone from a view, a move from an already-owned
/// value.
impl<'a> From<ValueView<'a>> for Value {
    fn from(view: ValueView<'a>) -> Self {
        view.to_owned()
    }
}

impl<'a> NodeView for ValueView<'a> {
    type Array = ArrayView<'a>;
    type File = FileView<'a>;
    type Object = ObjectView<'a>;
    type List = ListView<'a>;

    fn tag(&self) -> ValueTag {
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

    fn peel_secret(&self) -> &Self {
        let mut current = self;
        while let ValueView::Secret(inner) = current {
            current = inner;
        }
        current
    }

    fn as_bool(&self) -> Option<bool> {
        match self.peel_secret() {
            ValueView::Bool(b) => Some(*b),
            _ => None,
        }
    }

    fn as_int(&self) -> Option<i64> {
        match self.peel_secret() {
            ValueView::Int(i) => Some(*i),
            _ => None,
        }
    }

    fn as_float(&self) -> Option<f64> {
        match self.peel_secret() {
            ValueView::Float(f) => Some(*f),
            _ => None,
        }
    }

    // Trait `as_str` returns a `&self`-bound `&str`; `ValueView`'s inherent
    // `as_str` above returns the decoupled `&'a str` and shadows this for
    // direct calls. This impl exists so generic `T: NodeView` code works.
    fn as_str(&self) -> Option<&str> {
        match self.peel_secret() {
            ValueView::String(s) => Some(s),
            _ => None,
        }
    }

    fn as_array(&self) -> Option<&ArrayView<'a>> {
        match self.peel_secret() {
            ValueView::Array(a) => Some(a),
            _ => None,
        }
    }

    fn as_file(&self) -> Option<&FileView<'a>> {
        match self.peel_secret() {
            ValueView::File(f) => Some(f),
            _ => None,
        }
    }

    fn as_object(&self) -> Option<&ObjectView<'a>> {
        match self.peel_secret() {
            ValueView::Object(o) => Some(o),
            _ => None,
        }
    }

    fn as_list(&self) -> Option<&ListView<'a>> {
        match self.peel_secret() {
            ValueView::List(l) => Some(l),
            _ => None,
        }
    }
}

impl<'a> TryFrom<ValueView<'a>> for ObjectView<'a> {
    type Error = ValueView<'a>;

    /// Detach a borrowed object, moving the spine out — seeing through `Secret`
    /// wrappers like every other typed accessor. A value that isn't an object is
    /// handed back **with its secret wrapper intact**, so a failed detach never
    /// strips the redaction guard.
    fn try_from(value: ValueView<'a>) -> Result<Self, Self::Error> {
        match value {
            ValueView::Object(object) => Ok(object),
            ValueView::Secret(inner) => {
                ObjectView::try_from(*inner).map_err(|inner| ValueView::Secret(Box::new(inner)))
            }
            other => Err(other),
        }
    }
}

impl<'a> TryFrom<ValueView<'a>> for ListView<'a> {
    type Error = ValueView<'a>;

    /// Detach a borrowed list, moving the spine out — seeing through `Secret`
    /// wrappers like every other typed accessor. A value that isn't a list is
    /// handed back **with its secret wrapper intact**, so a failed detach never
    /// strips the redaction guard.
    fn try_from(value: ValueView<'a>) -> Result<Self, Self::Error> {
        match value {
            ValueView::List(list) => Ok(list),
            ValueView::Secret(inner) => {
                ListView::try_from(*inner).map_err(|inner| ValueView::Secret(Box::new(inner)))
            }
            other => Err(other),
        }
    }
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{parse, writer};

    #[test]
    fn a_view_materializes_into_an_owned_value() {
        let bytes = writer::to_bytes(Value::Object(Object::new(vec![(
            "k".into(),
            Value::String("v".into()),
        )])))
        .expect("serialize");
        let view = parse(&bytes).expect("parse");
        // `impl Into<Value>` on a view clones out of the buffer.
        let owned: Value = view.into();
        assert_eq!(owned.string("k").expect("string"), "v");
    }
}
