//! Object key rules, shared by the parser and the writer

use std::collections::HashMap;
use std::collections::hash_map::Entry;
use std::fmt;

/// A key that breaks one of the rules in [`check_keys`]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct KeyError<'k> {
    /// Index of the offending entry in its object
    pub index: usize,
    pub key: &'k str,
    pub problem: KeyProblem,
}

/// Which rule a key breaks
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum KeyProblem {
    /// Contains `/`, which separates path segments in `get`
    ContainsSlash,
    /// Repeats the key of the entry at index `first`, the only one `get`
    /// would find
    Duplicate { first: usize },
}

impl fmt::Display for KeyError<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let key = self.key;
        match self.problem {
            KeyProblem::ContainsSlash => write!(f, "key contains '/': {key:?}"),
            KeyProblem::Duplicate { first } => {
                write!(
                    f,
                    "duplicate key: {key:?} (entries {first} and {})",
                    self.index
                )
            }
        }
    }
}

/// Check an object's keys, in order: none may contain `/`, and none may repeat
pub(crate) fn check_keys<'k>(keys: impl IntoIterator<Item = &'k str>) -> Result<(), KeyError<'k>> {
    let mut first_index = HashMap::new();
    for (index, key) in keys.into_iter().enumerate() {
        let error = |problem| KeyError {
            index,
            key,
            problem,
        };
        if key.contains('/') {
            return Err(error(KeyProblem::ContainsSlash));
        }
        match first_index.entry(key) {
            Entry::Occupied(first) => {
                return Err(error(KeyProblem::Duplicate {
                    first: *first.get(),
                }));
            }
            Entry::Vacant(slot) => {
                slot.insert(index);
            }
        }
    }
    Ok(())
}
