//! Human-readable `Display` rendering for parsed values (used by tools like the
//! CLI). Distinct from `Debug`:
//!
//! - secrets render as `<redacted>` (the same redaction guarantee), and
//! - binary `File`/`Array` values render as a one-line summary instead of
//!   dumping their bytes.
//!
//! The layout is a simple YAML-ish indented tree.

use super::ValueView;
use super::value::REDACTED;
use std::fmt;

impl<'a> fmt::Display for ValueView<'a> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt_view(f, self, 0)
    }
}

/// True for non-empty containers, which render across multiple indented lines.
fn is_block(v: &ValueView<'_>) -> bool {
    match v {
        ValueView::Object(e) => !e.is_empty(),
        ValueView::List(items) => !items.is_empty(),
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

fn fmt_view(f: &mut fmt::Formatter<'_>, v: &ValueView<'_>, indent: usize) -> fmt::Result {
    match v {
        // Redaction choke point for Display, mirroring Debug.
        ValueView::Secret(_) => f.write_str(REDACTED),
        ValueView::Bool(b) => write!(f, "{b}"),
        ValueView::Int(i) => write!(f, "{i}"),
        ValueView::Float(x) => write!(f, "{x}"),
        ValueView::String(s) => fmt_string(f, s),
        ValueView::File(file) => {
            write!(
                f,
                "<file {}, {}>",
                file.mimetype,
                human_size(file.data.len() as u64)
            )
        }
        ValueView::Array(a) => {
            write!(
                f,
                "<array {:?} {:?}, {}>",
                a.dtype,
                a.shape,
                human_size(a.data.len() as u64)
            )
        }
        ValueView::Object(entries) if entries.is_empty() => f.write_str("{}"),
        ValueView::List(items) if items.is_empty() => f.write_str("[]"),
        ValueView::Object(entries) => {
            for (i, (key, val)) in entries.iter().enumerate() {
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
        ValueView::List(items) => {
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
fn fmt_member(f: &mut fmt::Formatter<'_>, val: &ValueView<'_>, indent: usize) -> fmt::Result {
    if is_block(val) {
        fmt_view(f, val, indent)
    } else {
        f.write_str(" ")?;
        fmt_view(f, val, indent)
    }
}

/// Strings render raw, except those containing control characters (newlines,
/// tabs, …) are quoted/escaped. This keeps each value on one line so a value
/// can't break the indented layout or forge a fake `key:` line in the output.
fn fmt_string(f: &mut fmt::Formatter<'_>, s: &str) -> fmt::Result {
    if s.chars().any(char::is_control) {
        write!(f, "{s:?}")
    } else {
        write!(f, "{s}")
    }
}

#[cfg(test)]
mod tests {
    use super::human_size;
    use crate::{Value, parse, writer};

    fn display_of(value: Value) -> String {
        let bytes = writer::to_bytes(value).unwrap();
        // The borrowed ValueView only needs to outlive the `format!` call, which
        // happens before `bytes` drops at the end of this statement.
        format!("{}", parse(&bytes).unwrap())
    }

    #[test]
    fn scalars_render_inline() {
        assert_eq!(display_of(Value::Bool(true)), "true");
        assert_eq!(display_of(Value::Int(42)), "42");
        assert_eq!(display_of(Value::String("hi".into())), "hi");
    }

    #[test]
    fn object_is_indented_tree() {
        let v = Value::Object(vec![
            ("name".into(), Value::String("test".into())),
            (
                "nested".into(),
                Value::Object(vec![("k".into(), Value::Int(1))]),
            ),
        ]);
        let out = display_of(v);
        // Root-level first entry has no leading blank line.
        assert!(out.starts_with("name: test"), "{out:?}");
        assert!(out.contains("\nnested:"), "{out:?}");
        assert!(out.contains("\n  k: 1"), "{out:?}");
    }

    #[test]
    fn secret_is_redacted_in_display() {
        let v = Value::Object(vec![("pw".into(), Value::secret("hunter2"))]);
        let out = display_of(v);
        assert!(out.contains("pw: <redacted>"), "{out:?}");
        assert!(!out.contains("hunter2"), "secret leaked: {out:?}");
    }

    #[test]
    fn secret_object_subtree_redacted_in_display() {
        let v = Value::Object(vec![(
            "db".into(),
            Value::secret(Value::Object(vec![(
                "token".into(),
                Value::String("s3cr3t".into()),
            )])),
        )]);
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
        assert_eq!(display_of(Value::Object(vec![])), "{}");
        assert_eq!(display_of(Value::List(vec![])), "[]");
    }

    #[test]
    fn list_renders_with_dashes() {
        let v = Value::List(vec![Value::Int(1), Value::String("a".into())]);
        assert_eq!(display_of(v), "- 1\n- a");
    }

    #[test]
    fn control_chars_are_escaped_to_one_line() {
        // A value must not be able to forge a fake `key:` line by embedding a
        // newline. The escaped form stays on a single line.
        let v = Value::Object(vec![(
            "note".into(),
            Value::String("x\nadmin: true".into()),
        )]);
        let out = display_of(v);
        assert!(
            !out.contains("\nadmin: true"),
            "value forged a line: {out:?}"
        );
        assert!(out.starts_with("note: \""), "{out:?}");
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
