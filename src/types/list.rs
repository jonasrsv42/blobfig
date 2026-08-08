//! List payloads — an ordered sequence of items, owned ([`List`]) or borrowed
//! ([`ListView`]). Read generically through [`ListNode`](super::ListNode):
//! borrow the items in place with `items`, or consume the list with
//! [`IntoIterator`] to move them out.

use std::ops::Deref;

use super::node_view::ListNode;
use super::{Value, ValueView};

/// Owned list: items in order.
#[derive(Default)]
pub struct List(Vec<Value>);

impl List {
    /// A list from its items.
    pub fn new(items: Vec<Value>) -> Self {
        List(items)
    }
}

impl From<Vec<Value>> for List {
    fn from(items: Vec<Value>) -> Self {
        List(items)
    }
}

// Deref to the items slice, so `len`/`is_empty`/`iter`/indexing read like a
// slice without re-exposing the `Vec`.
impl Deref for List {
    type Target = [Value];
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl IntoIterator for List {
    type Item = Value;
    type IntoIter = std::vec::IntoIter<Value>;
    fn into_iter(self) -> Self::IntoIter {
        self.0.into_iter()
    }
}

// By-ref iteration, so `for item in &list` works (the consuming `IntoIterator`
// above covers `for item in list`).
impl<'a> IntoIterator for &'a List {
    type Item = &'a Value;
    type IntoIter = std::slice::Iter<'a, Value>;
    fn into_iter(self) -> Self::IntoIter {
        self.0.iter()
    }
}

impl ListNode for List {
    type Node = Value;
    fn items(&self) -> &[Value] {
        &self.0
    }
}

// Delegate to the items so `Value::List(..)` prints as `List([..])` — the shape
// a derived `Debug` on the enum produces.
impl std::fmt::Debug for List {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

/// Borrowed list: items referencing the source buffer.
#[derive(Clone)]
pub struct ListView<'a>(Vec<ValueView<'a>>);

impl<'a> ListView<'a> {
    /// A list view from its items.
    pub fn new(items: Vec<ValueView<'a>>) -> Self {
        ListView(items)
    }
}

impl<'a> From<Vec<ValueView<'a>>> for ListView<'a> {
    fn from(items: Vec<ValueView<'a>>) -> Self {
        ListView(items)
    }
}

impl<'a> Deref for ListView<'a> {
    type Target = [ValueView<'a>];
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<'view, 'a> IntoIterator for &'view ListView<'a> {
    type Item = &'view ValueView<'a>;
    type IntoIter = std::slice::Iter<'view, ValueView<'a>>;
    fn into_iter(self) -> Self::IntoIter {
        self.0.iter()
    }
}

impl<'a> IntoIterator for ListView<'a> {
    type Item = ValueView<'a>;
    type IntoIter = std::vec::IntoIter<ValueView<'a>>;
    fn into_iter(self) -> Self::IntoIter {
        self.0.into_iter()
    }
}

impl<'a> ListNode for ListView<'a> {
    type Node = ValueView<'a>;
    fn items(&self) -> &[ValueView<'a>] {
        &self.0
    }
}

impl<'a> std::fmt::Debug for ListView<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}
