//! How deep values may nest, shared by the parser and the writer

/// How many values deep a blob may nest, counting every value from the root
/// down to the innermost one, `Secret` wrappers included
///
/// The parser refuses deeper input with
/// [`ParseErrorReason::TooDeep`](crate::ParseErrorReason) instead of
/// overflowing the stack, and the writer refuses to write it.
///
/// Measured in a release build on x86-64, parsing one nested object or list
/// takes about 1.8 KB of stack, so 12 levels take about 22 KB, leaving room on
/// a 32 KiB task such as an ESP32's main task. The parser's depth counter is
/// per thread, shared with any other parsicomb `max_depth` grammar running on
/// it.
pub const MAX_DEPTH: usize = 12;
