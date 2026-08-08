//! Object payloads — an ordered set of `(key, value)` entries, owned
//! ([`Object`]) or borrowed ([`ObjectView`]). Read generically through
//! [`ObjectNode`](super::ObjectNode): borrow the entries in place with
//! `entries`, or consume the object with [`IntoIterator`] to move them out.
//!
//! The concrete types also `Deref` to their entry storage — the owned
//! [`Object`] to its `Vec` (and `DerefMut`, so it can be built up in place), the
//! borrowed [`ObjectView`] to a read-only slice — so direct code has the
//! `Vec`-like ergonomics (`len`, indexing, `push`) alongside the trait accessors.

use std::ops::{Deref, DerefMut};

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
}

impl Deref for Object {
    type Target = Vec<(String, Value)>;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl DerefMut for Object {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
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
}

impl<'a> Deref for ObjectView<'a> {
    type Target = [(&'a str, ValueView<'a>)];
    fn deref(&self) -> &Self::Target {
        &self.0
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn owned_object_derefs_to_its_entries_and_is_mutable() {
        let mut object = Object::new(vec![("a".to_owned(), Value::Int(1))]);
        assert_eq!(object.len(), 1); // Deref -> Vec::len
        assert_eq!(object[0].0, "a"); // Deref -> indexing
        object.push(("b".to_owned(), Value::Int(2))); // DerefMut -> Vec::push
        assert_eq!(object.len(), 2);
        assert_eq!(object[1].0, "b");
    }

    #[test]
    fn borrowed_object_view_derefs_to_a_read_only_slice() {
        let view = ObjectView::new(vec![("k", ValueView::Int(1))]);
        assert_eq!(view.len(), 1);
        assert_eq!(view[0].0, "k");
    }
}
