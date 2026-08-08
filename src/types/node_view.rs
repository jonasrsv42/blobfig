//! The view-like read interface shared by owned [`Value`](super::Value) and
//! borrowed [`ValueView`](super::ValueView).
//!
//! The two are the same enum shape; they differ only in whether their payloads
//! are owned (`String`, `Array`, …) or borrow the source buffer (`&str`,
//! `ArrayView`, …). Reading a node by reference — peeking a leaf, walking a
//! path, rendering it — doesn't care about that distinction, so it lives here
//! once on the [`NodeView`] trait. Each type supplies only the small,
//! variant-matching primitives; the traversal and the typed, path-aware
//! accessors are provided by the trait.
//!
//! # The contract: the methods read; ownership rides on the supertraits
//!
//! Every *method* here borrows (`&self`) and returns a borrow or a `Copy`
//! scalar — the most either representation can promise in common, since a
//! `ValueView` owns nothing a method could hand out. Moving is therefore never
//! a method; it rides on supertrait bounds, each doing the cheapest thing per
//! representation:
//!
//! - materialize the whole node to an owned [`Value`](super::Value) →
//!   [`Into<Value>`]: a move for an owned `Value`, a clone for a borrowed
//!   `ValueView`.
//! - detach a container payload → the [`TryInto`] supertraits: they move the
//!   [`Object`](super::Object)/[`List`](super::List) out (through any `Secret`
//!   wrapper), or hand the node back intact — secret and all — when it isn't
//!   that container.
//! - split a container into its entries/items → the payload's [`IntoIterator`],
//!   moving them out in one pass.
//!
//! Navigation by path (`get`, `float`, `string`, …) stays borrowing — you
//! cannot move a leaf out of a structure you hold by reference.
//!
//! Bring the trait into scope (`use blobfig::NodeView;`) to call these methods
//! on either type.

use super::{DType, Value, ValueTag};
use crate::error::AccessError;

/// The read interface over a file leaf, shared by owned [`File`](super::File)
/// and borrowed [`FileView`](super::FileView). Bounds
/// [`NodeView::File`](NodeView::File) so generic tree code can read a file's
/// mimetype and size without naming the concrete type.
pub trait FileNode {
    /// The file's mimetype.
    fn mimetype(&self) -> &str;
    /// The file's size in bytes.
    fn size(&self) -> u64;
}

/// The read interface over an array leaf, shared by owned
/// [`Array`](super::Array) and borrowed [`ArrayView`](super::ArrayView). Bounds
/// [`NodeView::Array`](NodeView::Array) so generic tree code can read an array's
/// element type, shape, and raw bytes without naming the concrete type.
pub trait ArrayNode {
    /// The element type.
    fn dtype(&self) -> DType;
    /// The shape — one length per dimension.
    fn shape(&self) -> &[u64];
    /// The raw element bytes.
    fn data(&self) -> &[u8];
}

/// The read interface over an object payload, shared by owned
/// [`Object`](super::Object) and borrowed [`ObjectView`](super::ObjectView).
/// Bounds [`NodeView::Object`](NodeView::Object) so generic tree code can read
/// an object's entries: [`entries`](Self::entries) borrows them in place, while
/// consuming the object through its [`IntoIterator`] supertrait moves them out
/// in one pass — from a borrowed object the spine moves while keys stay
/// borrowed (`&'a str`).
pub trait ObjectNode: IntoIterator<Item = (Self::Key, Self::Node)> {
    /// The entry-value node: `Value` (owned) or `ValueView<'a>` (borrowed).
    type Node: NodeView;
    /// The key type when the object is consumed: `String` (owned) or `&'a str`
    /// (borrowed — matched without a copy).
    type Key: AsRef<str>;
    /// Borrow the entries in order, keys uniformly as `&str`.
    fn entries(&self) -> impl Iterator<Item = (&str, &Self::Node)>;
}

/// The read interface over a list payload, shared by owned [`List`](super::List)
/// and borrowed [`ListView`](super::ListView). Bounds
/// [`NodeView::List`](NodeView::List) so generic tree code can read a list's
/// items: [`items`](Self::items) borrows them, while consuming the list through
/// its [`IntoIterator`] supertrait moves them out.
pub trait ListNode: IntoIterator<Item = Self::Node> {
    /// The item node: `Value` (owned) or `ValueView<'a>` (borrowed).
    type Node: NodeView;
    /// Borrow the items in order.
    fn items(&self) -> &[Self::Node];
}

/// The view-like read interface over a value node, implemented by owned
/// [`Value`](super::Value) and borrowed [`ValueView`](super::ValueView).
///
/// The methods hold only borrowing reads (see the module docs for why move
/// semantics cannot live *among the methods*). Implementors provide the
/// variant-matching primitives; the path traversal and the typed
/// `Result`-returning accessors come for free as provided methods.
///
/// Materialization to an owned value lives on the [`Into<Value>`] supertrait
/// bound rather than as a method: every node can be turned into an owned
/// [`Value`] at the cheapest cost its representation allows — a move for an
/// already-owned `Value`, a clone for a borrowed `ValueView`. The [`TryInto`]
/// supertraits detach a container payload the same way: they move the
/// [`Object`](super::Object)/[`List`](super::List) out (owned or borrowed),
/// seeing through `Secret` wrappers like the read accessors do, or hand the node
/// back intact — secret and all — via `Error = Self` when it isn't that
/// container. That `Error` is a control-flow channel (the node you passed in),
/// not an error type.
///
/// These supertrait bounds also seal the trait in practice: a new implementor
/// must supply coherent [`Into<Value>`] and [`TryInto`] conversions against its
/// own payload types, not merely the read methods.
pub trait NodeView:
    Sized + Into<Value> + TryInto<Self::Object, Error = Self> + TryInto<Self::List, Error = Self>
{
    /// The array payload: `Array` (owned) or `ArrayView<'a>` (borrowed). Read
    /// through the [`ArrayNode`] interface, so generic code sees its dtype,
    /// shape, and bytes without naming the concrete type.
    type Array: ArrayNode;
    /// The file payload: `File` (owned) or `FileView<'a>` (borrowed). Read
    /// through the [`FileNode`] interface, so generic code sees its mimetype
    /// and size without naming the concrete type.
    type File: FileNode;
    /// The object payload: `Object` (owned) or `ObjectView<'a>` (borrowed).
    /// Read through the [`ObjectNode`] interface; its entries are themselves
    /// `Self`, so traversal stays in one node type.
    type Object: ObjectNode<Node = Self>;
    /// The list payload: `List` (owned) or `ListView<'a>` (borrowed). Read
    /// through the [`ListNode`] interface; its items are themselves `Self`.
    type List: ListNode<Node = Self>;

    // =========================================================================
    // Primitives — implemented per type (small variant matches)
    // =========================================================================

    /// Tag for this value's variant.
    fn tag(&self) -> ValueTag;

    /// Peel any `Secret` wrappers, returning the inner node (returns `self`
    /// when not secret). Lets explicit, typed access see through secrets while
    /// printing stays redacted.
    fn peel_secret(&self) -> &Self;

    /// As a bool (sees through a secret wrapper).
    fn as_bool(&self) -> Option<bool>;
    /// As an i64 (sees through a secret wrapper).
    fn as_int(&self) -> Option<i64>;
    /// As an f64 (sees through a secret wrapper).
    fn as_float(&self) -> Option<f64>;
    /// As a string (sees through a secret wrapper).
    fn as_str(&self) -> Option<&str>;
    /// As an array (sees through a secret wrapper).
    fn as_array(&self) -> Option<&Self::Array>;
    /// As a file (sees through a secret wrapper).
    fn as_file(&self) -> Option<&Self::File>;
    /// As an object (sees through a secret wrapper).
    fn as_object(&self) -> Option<&Self::Object>;
    /// As a list (sees through a secret wrapper).
    fn as_list(&self) -> Option<&Self::List>;

    // =========================================================================
    // Provided — written once, shared by every implementor
    // =========================================================================

    /// Look up a single key in an object, peeling secrets first so a secret
    /// sub-object can still be descended into.
    fn child(&self, key: &str) -> Option<&Self> {
        self.peel_secret()
            .as_object()?
            .entries()
            .find(|(k, _)| *k == key)
            .map(|(_, v)| v)
    }

    /// Get a nested value by path (e.g. `"audio/sample_rate"`).
    ///
    /// Traversal sees through `Secret` wrappers, so `get("db/password")` works
    /// even when `db` is secret. The returned node is left as-is, so a secret
    /// leaf comes back wrapped and still redacts when printed.
    fn get(&self, path: &str) -> Option<&Self> {
        let mut current = self;
        for key in path.split('/') {
            current = current.child(key)?;
        }
        Some(current)
    }

    /// Get a bool at path.
    fn bool(&self, path: &str) -> Result<bool, AccessError> {
        let value = self.require(path)?;
        value.as_bool().ok_or_else(|| value.mismatch(path, "bool"))
    }

    /// Get an i64 at path.
    fn int(&self, path: &str) -> Result<i64, AccessError> {
        let value = self.require(path)?;
        value.as_int().ok_or_else(|| value.mismatch(path, "int"))
    }

    /// Get an f64 at path.
    fn float(&self, path: &str) -> Result<f64, AccessError> {
        let value = self.require(path)?;
        value
            .as_float()
            .ok_or_else(|| value.mismatch(path, "float"))
    }

    /// Get a string at path.
    fn string(&self, path: &str) -> Result<&str, AccessError> {
        let value = self.require(path)?;
        value.as_str().ok_or_else(|| value.mismatch(path, "string"))
    }

    /// Get an array at path.
    fn array(&self, path: &str) -> Result<&Self::Array, AccessError> {
        let value = self.require(path)?;
        value
            .as_array()
            .ok_or_else(|| value.mismatch(path, "array"))
    }

    /// Get a file at path.
    fn file(&self, path: &str) -> Result<&Self::File, AccessError> {
        let value = self.require(path)?;
        value.as_file().ok_or_else(|| value.mismatch(path, "file"))
    }

    /// Resolve a path or report `NotFound` — the shared first half of every
    /// typed accessor above.
    fn require(&self, path: &str) -> Result<&Self, AccessError> {
        self.get(path).ok_or_else(|| AccessError::NotFound {
            path: path.to_string(),
        })
    }

    /// Build a `TypeMismatch` for `self` at `path` — the shared error tail of
    /// every typed accessor above. Reports the peeled tag so the message names
    /// the underlying type, not `Secret`.
    fn mismatch(&self, path: &str, expected: &'static str) -> AccessError {
        AccessError::TypeMismatch {
            path: path.to_string(),
            expected,
            actual: self.peel_secret().tag(),
        }
    }
}
