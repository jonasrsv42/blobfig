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
//! # The contract: this trait holds *only* view-like reads
//!
//! Every method here borrows (`&self`) and returns a borrow or a `Copy` scalar.
//! That is the most either representation can promise in common: a `ValueView`
//! owns nothing it could hand out, so move semantics are deliberately **not**
//! here and cannot be added — a trait method body must work for both types, and
//! a borrowed view could only satisfy an `into_*` by cloning, which would be a
//! move-shaped lie. Ownership lives where it actually exists:
//!
//! - moving a payload *out* of an owned value → `into_*` / `remove` inherent on
//!   [`Value`](super::Value) only;
//! - materializing an owned copy *from* a borrowed view → `to_owned` inherent on
//!   [`ValueView`](super::ValueView) (an honest clone, per the `to_*` convention).
//!
//! Two naming families, each internally honest: conversion of `self` is `as_*`
//! (borrow) vs `into_*` (own); navigation by path (`get`, `float`, `string`, …)
//! is always borrowing — you cannot move a leaf out of a structure you hold by
//! reference, so the path family has no ownership axis to encode.
//!
//! Bring the trait into scope (`use blobfig::NodeView;`) to call these methods
//! on either type.

use super::{DType, ValueTag};
use crate::error::AccessError;

/// The view-like read interface over a value node, implemented by owned
/// [`Value`](super::Value) and borrowed [`ValueView`](super::ValueView).
///
/// Holds only borrowing reads (see the module docs for why move semantics
/// cannot live here). Implementors provide the variant-matching primitives; the
/// path traversal and the typed `Result`-returning accessors come for free as
/// provided methods.
pub trait NodeView: Sized {
    /// The array payload: `Array` (owned) or `ArrayView<'a>` (borrowed).
    type Array;
    /// The file payload: `File` (owned) or `FileView<'a>` (borrowed).
    type File;

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
    /// As a list (sees through a secret wrapper).
    fn as_list(&self) -> Option<&[Self]>;

    /// Object entries as `(key, value)` pairs, hiding the key-type difference
    /// (`&str` vs `String`) behind a uniform iterator so traversal and
    /// `Display` can be written once. Does **not** peel secrets — callers that
    /// want to descend through a secret object peel first (see [`child`]).
    ///
    /// [`child`]: NodeView::child
    fn object_entries(&self) -> Option<impl Iterator<Item = (&str, &Self)>>;

    /// `(mimetype, byte_len)` for a file leaf — the data `Display` summarizes
    /// without materializing the bytes (sees through a secret wrapper).
    fn as_file_summary(&self) -> Option<(&str, u64)>;
    /// `(dtype, shape, byte_len)` for an array leaf — the data `Display`
    /// summarizes (sees through a secret wrapper).
    fn as_array_summary(&self) -> Option<(DType, &[u64], u64)>;

    // =========================================================================
    // Provided — written once, shared by every implementor
    // =========================================================================

    /// Look up a single key in an object, peeling secrets first so a secret
    /// sub-object can still be descended into.
    fn child(&self, key: &str) -> Option<&Self> {
        self.peel_secret()
            .object_entries()?
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
