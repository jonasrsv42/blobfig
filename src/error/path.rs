//! Where in a value tree something is

use std::fmt;

/// One step into a container: an object's key or a list's index
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PathSegment {
    Key(String),
    Index(usize),
}

impl fmt::Display for PathSegment {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PathSegment::Key(key) => f.write_str(key),
            PathSegment::Index(index) => write!(f, "{index}"),
        }
    }
}

/// The steps from the root value down to a value inside it; empty for the
/// root itself
///
/// Displays as its segments joined by `/`, as in `model/layers/3/weights`.
/// Through objects only, that is the path `get` takes. A key `"3"` and an
/// index `3` display the same; [`segments`](ValuePath::segments) tells them
/// apart.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ValuePath {
    /// Innermost first, since an error learns its path on the way out of the
    /// tree
    reversed: Vec<PathSegment>,
}

impl ValuePath {
    /// True for the root value
    pub fn is_root(&self) -> bool {
        self.reversed.is_empty()
    }

    /// The segments, from the root down
    pub fn segments(&self) -> impl DoubleEndedIterator<Item = &PathSegment> + ExactSizeIterator {
        self.reversed.iter().rev()
    }

    /// Put `parent` in front, for a path learned from the inside out
    pub(crate) fn prepend(&mut self, parent: PathSegment) {
        self.reversed.push(parent);
    }

    /// `message`, saying where it happened: `at "a/b": message`, or just
    /// `message` at the root. The path is quoted, so keys with spaces or
    /// newlines, and empty keys, show.
    pub fn locate<M: fmt::Display>(&self, message: M) -> impl fmt::Display {
        Located(self, message)
    }
}

/// From the segments, root first
impl FromIterator<PathSegment> for ValuePath {
    fn from_iter<I: IntoIterator<Item = PathSegment>>(segments: I) -> Self {
        let mut reversed: Vec<_> = segments.into_iter().collect();
        reversed.reverse();
        ValuePath { reversed }
    }
}

struct Located<'p, M>(&'p ValuePath, M);

impl<M: fmt::Display> fmt::Display for Located<'_, M> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let Located(path, message) = self;
        if path.is_root() {
            return message.fmt(f);
        }
        write!(f, "at {:?}: {message}", path.to_string())
    }
}

impl fmt::Display for ValuePath {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for (i, segment) in self.segments().enumerate() {
            if i > 0 {
                f.write_str("/")?;
            }
            segment.fmt(f)?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn collects_from_the_root_down() {
        let segments = [PathSegment::Key("a".into()), PathSegment::Index(0)];
        let path: ValuePath = segments.clone().into_iter().collect();
        assert!(path.segments().eq(&segments));
    }
}
