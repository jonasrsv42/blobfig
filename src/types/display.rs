//! Human-readable `Display` rendering for values (used by tools like the CLI).
//! Distinct from `Debug`:
//!
//! - secrets render as `<redacted>` (the same redaction guarantee), and
//! - binary `File`/`Array` values render as a one-line summary instead of
//!   dumping their bytes.
//!
//! The layout is a simple YAML-ish indented tree. The walker is written once
//! over [`NodeView`] so owned [`Value`] and borrowed [`ValueView`] share it.

use super::value::REDACTED;
use super::{ArrayNode, FileNode, ListNode, NodeView, ObjectNode, Value, ValueTag, ValueView};
use std::fmt;

/// Rendered when a leaf's tag and its accessor disagree — a "can't happen"
/// consistency break. Shown loudly (never a plausible `0`/`false`/`""`) so the
/// drift is visible rather than silently formatted as real data.
const DISPLAY_ERROR: &str = "<display_error>";

impl fmt::Display for Value {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt_node(f, self, 0)
    }
}

impl<'a> fmt::Display for ValueView<'a> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt_node(f, self, 0)
    }
}

/// True for non-empty containers, which render across multiple indented lines.
/// Secrets are never blocks — they redact inline.
fn is_block<T: NodeView>(v: &T) -> bool {
    match v.tag() {
        ValueTag::Object => v.as_object().is_some_and(|o| o.entries().next().is_some()),
        ValueTag::List => v.as_list().is_some_and(|l| !l.items().is_empty()),
        _ => false,
    }
}

fn human_size(bytes: u64) -> String {
    const UNITS: [&str; 5] = ["B", "KiB", "MiB", "GiB", "TiB"];
    let mut size = bytes as f64;
    let mut unit = 0;
    while size >= 1024.0 && unit < UNITS.len() - 1 {
        size /= 1024.0;
        unit += 1;
    }
    if unit == 0 {
        format!("{bytes} B")
    } else {
        format!("{size:.1} {}", UNITS[unit])
    }
}

fn fmt_node<T: NodeView>(f: &mut fmt::Formatter<'_>, v: &T, indent: usize) -> fmt::Result {
    // Match on the tag so this stays exhaustive — a new variant won't silently
    // render as nothing. The tag selects the arm, so each accessor below is
    // guaranteed `Some` (the `as_*` peel secrets, but a `Secret` tag is handled
    // first, mirroring `Debug`'s redaction choke point). If the tag and its
    // accessor ever disagree, each arm renders `DISPLAY_ERROR` rather than
    // unwrapping — visible, and panic-free.
    match v.tag() {
        ValueTag::Secret => f.write_str(REDACTED),
        ValueTag::Bool => match v.as_bool() {
            Some(b) => write!(f, "{b}"),
            None => f.write_str(DISPLAY_ERROR),
        },
        ValueTag::Int => match v.as_int() {
            Some(i) => write!(f, "{i}"),
            None => f.write_str(DISPLAY_ERROR),
        },
        ValueTag::Float => match v.as_float() {
            Some(x) => write!(f, "{x}"),
            None => f.write_str(DISPLAY_ERROR),
        },
        // Quoted (Rust debug form) so strings are unambiguous — `"true"` vs the
        // bool `true`, `"42"` vs the int `42` — and so control characters are
        // escaped, keeping each value on one line (can't forge a fake `key:`).
        ValueTag::String => match v.as_str() {
            Some(s) => write!(f, "{s:?}"),
            None => f.write_str(DISPLAY_ERROR),
        },
        ValueTag::File => match v.as_file() {
            Some(file) => write!(f, "<file {}, {}>", file.mimetype(), human_size(file.size())),
            None => f.write_str(DISPLAY_ERROR),
        },
        ValueTag::Array => match v.as_array() {
            Some(array) => write!(
                f,
                "<array {:?} {:?}, {}>",
                array.dtype(),
                array.shape(),
                human_size(array.data().len() as u64)
            ),
            None => f.write_str(DISPLAY_ERROR),
        },
        ValueTag::Object => {
            let Some(object) = v.as_object() else {
                return f.write_str(DISPLAY_ERROR);
            };
            if object.entries().next().is_none() {
                return f.write_str("{}");
            }
            for (i, (key, val)) in object.entries().enumerate() {
                // Root-level first entry has no leading newline (avoids a blank
                // line); everything else starts a fresh indented line.
                if indent == 0 && i == 0 {
                    write!(f, "{key}:")?;
                } else {
                    write!(f, "\n{:indent$}{key}:", "")?;
                }
                fmt_member(f, val, indent + 2)?;
            }
            Ok(())
        }
        ValueTag::List => {
            let Some(list) = v.as_list() else {
                return f.write_str(DISPLAY_ERROR);
            };
            let items = list.items();
            if items.is_empty() {
                return f.write_str("[]");
            }
            for (i, item) in items.iter().enumerate() {
                if indent == 0 && i == 0 {
                    f.write_str("-")?;
                } else {
                    write!(f, "\n{:indent$}-", "")?;
                }
                fmt_member(f, item, indent + 2)?;
            }
            Ok(())
        }
    }
}

/// Render a member value after its `key:` / `-` marker: blocks continue on the
/// following indented lines, scalars stay inline after a single space.
fn fmt_member<T: NodeView>(f: &mut fmt::Formatter<'_>, val: &T, indent: usize) -> fmt::Result {
    if is_block(val) {
        fmt_node(f, val, indent)
    } else {
        f.write_str(" ")?;
        fmt_node(f, val, indent)
    }
}

#[cfg(test)]
mod tests {
    use super::human_size;
    use crate::{List, Object, Value, parse, writer};

    fn display_of(value: Value) -> String {
        let bytes = writer::to_bytes(value).unwrap();
        // The borrowed ValueView only needs to outlive the `format!` call, which
        // happens before `bytes` drops at the end of this statement.
        format!("{}", parse(&bytes).unwrap())
    }

    /// Owned `Value` and the parsed `ValueView` must render identically — the
    /// walker is shared, this guards the two `Display` impls against drift.
    fn assert_same_display(value: Value) {
        let owned = format!("{}", value);
        let viewed = display_of(value);
        assert_eq!(owned, viewed, "owned vs viewed Display drift");
    }

    #[test]
    fn scalars_render_inline() {
        assert_eq!(display_of(Value::Bool(true)), "true");
        assert_eq!(display_of(Value::Int(42)), "42");
        // Strings are quoted to stay unambiguous against bool/int.
        assert_eq!(display_of(Value::String("hi".into())), "\"hi\"");
        assert_eq!(display_of(Value::String("true".into())), "\"true\"");
        assert_eq!(display_of(Value::String("42".into())), "\"42\"");
    }

    #[test]
    fn object_is_indented_tree() {
        let v = Value::Object(Object::new(vec![
            ("name".into(), Value::String("test".into())),
            (
                "nested".into(),
                Value::Object(Object::new(vec![("k".into(), Value::Int(1))])),
            ),
        ]));
        let out = display_of(v);
        // Root-level first entry has no leading blank line.
        assert!(out.starts_with("name: \"test\""), "{out:?}");
        assert!(out.contains("\nnested:"), "{out:?}");
        assert!(out.contains("\n  k: 1"), "{out:?}");
    }

    #[test]
    fn secret_is_redacted_in_display() {
        let v = Value::Object(Object::new(vec![("pw".into(), Value::secret("hunter2"))]));
        let out = display_of(v);
        assert!(out.contains("pw: <redacted>"), "{out:?}");
        assert!(!out.contains("hunter2"), "secret leaked: {out:?}");
    }

    #[test]
    fn secret_object_subtree_redacted_in_display() {
        let v = Value::Object(Object::new(vec![(
            "db".into(),
            Value::secret(Value::Object(Object::new(vec![(
                "token".into(),
                Value::String("s3cr3t".into()),
            )]))),
        )]));
        let out = display_of(v);
        assert!(out.contains("db: <redacted>"), "{out:?}");
        assert!(!out.contains("s3cr3t"), "secret leaked: {out:?}");
    }

    #[test]
    fn file_renders_as_summary_not_bytes() {
        use crate::types::File;
        let v = Value::File(File::from_bytes("application/x-tflite", vec![0u8; 2048]));
        let out = display_of(v);
        assert!(
            out.contains("<file application/x-tflite, 2.0 KiB>"),
            "{out:?}"
        );
    }

    #[test]
    fn empty_containers() {
        assert_eq!(display_of(Value::Object(Object::new(vec![]))), "{}");
        assert_eq!(display_of(Value::List(List::new(vec![]))), "[]");
    }

    #[test]
    fn list_renders_with_dashes() {
        let v = Value::List(List::new(vec![Value::Int(1), Value::String("a".into())]));
        assert_eq!(display_of(v), "- 1\n- \"a\"");
    }

    #[test]
    fn control_chars_are_escaped_to_one_line() {
        // A value must not be able to forge a fake `key:` line by embedding a
        // newline. The escaped form stays on a single line.
        let v = Value::Object(Object::new(vec![(
            "note".into(),
            Value::String("x\nadmin: true".into()),
        )]));
        let out = display_of(v);
        assert!(
            !out.contains("\nadmin: true"),
            "value forged a line: {out:?}"
        );
        assert!(out.starts_with("note: \""), "{out:?}");
    }

    #[test]
    fn owned_and_viewed_display_match() {
        // Covers scalars, nested objects, lists, files and secrets in one tree.
        assert_same_display(Value::Object(Object::new(vec![
            ("name".into(), Value::String("test".into())),
            ("count".into(), Value::Int(3)),
            (
                "nested".into(),
                Value::Object(Object::new(vec![("flag".into(), Value::Bool(true))])),
            ),
            (
                "items".into(),
                Value::List(List::new(vec![Value::Int(1), Value::String("a".into())])),
            ),
            ("pw".into(), Value::secret("hunter2")),
        ])));
    }

    #[test]
    fn human_size_units() {
        assert_eq!(human_size(0), "0 B");
        assert_eq!(human_size(1023), "1023 B");
        assert_eq!(human_size(1024), "1.0 KiB");
        assert_eq!(human_size(1536), "1.5 KiB");
        assert_eq!(human_size(1024 * 1024), "1.0 MiB");
    }
}
