//! Object payloads — an ordered set of `(key, value)` entries, owned
//! ([`Object`]) or borrowed ([`ObjectView`]). Read generically through
//! [`ObjectNode`](super::ObjectNode): borrow the entries in place with
//! `entries`, or consume the object with [`IntoIterator`] to move them out.

use super::node_view::ObjectNode;
use super::{Value, ValueView};

/// Owned object: `(key, value)` entries in insertion order.
#[derive(Default)]
pub struct Object(Vec<(String, Value)>);

impl Object {
    /// An object from its entries.
    pub fn new(entries: Vec<(String, Value)>) -> Self {
        Object(entries)
    }

    /// Number of entries.
    pub fn len(&self) -> usize {
        self.0.len()
    }

    /// Whether the object has no entries.
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

impl From<Vec<(String, Value)>> for Object {
    fn from(entries: Vec<(String, Value)>) -> Self {
        Object(entries)
    }
}

impl IntoIterator for Object {
    type Item = (String, Value);
    type IntoIter = std::vec::IntoIter<(String, Value)>;
    fn into_iter(self) -> Self::IntoIter {
        self.0.into_iter()
    }
}

impl ObjectNode for Object {
    type Node = Value;
    type Key = String;
    fn entries(&self) -> impl Iterator<Item = (&str, &Value)> {
        self.0.iter().map(|(key, value)| (key.as_str(), value))
    }
}

// Delegate to the entries so `Value::Object(..)` prints as
// `Object([("k", v), ..])` — the shape a derived `Debug` on the enum produces.
impl std::fmt::Debug for Object {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

/// Borrowed object: `(key, value)` entries referencing the source buffer.
#[derive(Clone)]
pub struct ObjectView<'a>(Vec<(&'a str, ValueView<'a>)>);

impl<'a> ObjectView<'a> {
    /// An object view from its entries.
    pub fn new(entries: Vec<(&'a str, ValueView<'a>)>) -> Self {
        ObjectView(entries)
    }

    /// Number of entries.
    pub fn len(&self) -> usize {
        self.0.len()
    }

    /// Whether the object has no entries.
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

impl<'a> From<Vec<(&'a str, ValueView<'a>)>> for ObjectView<'a> {
    fn from(entries: Vec<(&'a str, ValueView<'a>)>) -> Self {
        ObjectView(entries)
    }
}

impl<'a> IntoIterator for ObjectView<'a> {
    type Item = (&'a str, ValueView<'a>);
    type IntoIter = std::vec::IntoIter<(&'a str, ValueView<'a>)>;
    fn into_iter(self) -> Self::IntoIter {
        self.0.into_iter()
    }
}

impl<'a> ObjectNode for ObjectView<'a> {
    type Node = ValueView<'a>;
    type Key = &'a str;
    fn entries(&self) -> impl Iterator<Item = (&str, &ValueView<'a>)> {
        self.0.iter().map(|(key, value)| (*key, value))
    }
}

impl<'a> std::fmt::Debug for ObjectView<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}
