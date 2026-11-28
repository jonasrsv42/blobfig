# blobfig

<!-- The `rust` examples here are doctests: src/lib.rs includes this file, so
`cargo test --doc --all-features` runs them. Mark every other block (`text`,
`toml`, ...). The `text` output blocks are not checked by anything: when you
change an example, run it and paste what it prints. -->

A binary format that bundles configuration, typed arrays and files (such as
ML models) into one blob, parsed without copying: strings, arrays and files
are slices of the input.

```rust
use blobfig::{Array, DType, File, NodeView, Value, parse, writer};

let config = Value::object(vec![
    ("sample_rate".into(), Value::Int(16000)),
    ("api_key".into(), Value::secret("hunter2")),
    ("mean".into(), Value::Array(Array::new(DType::F32, vec![3], vec![0; 12]))),
    ("model".into(), Value::File(File::from_bytes("application/x-tflite", vec![0; 2048]))),
]);
let bytes = writer::to_bytes(config).unwrap();

let parsed = parse(&bytes).unwrap();
assert_eq!(parsed.int("sample_rate").unwrap(), 16000);
println!("{parsed}");
```

prints

```text
sample_rate: 16000
api_key: <redacted>
mean: <array F32 [3], 12 B>
model: <file application/x-tflite, 2.0 KiB>
```

- **One artifact:** config, arrays with a dtype and shape, and files with a
  mimetype travel together, so a model and its preprocessing can't go out of
  sync.
- **Zero-copy parsing:** `parse` borrows from the input, so reading a value
  out of a memory-mapped blob costs no copy, however large the model in it.
- **Arrays you can use in place:** array and file data are
  [aligned to 64 bytes](#arrays-without-copying), so an mmap'd array is an
  ndarray view.
- **Secrets that stay out of logs:** a value marked [secret](#secrets) prints
  as `<redacted>`, and is still readable by code that asks for it.
- **Errors your code can act on:** a [parse error](#errors) has an offset, a
  reason to match on, and a class that says what to do about it.
- **Safe on untrusted input:** nothing panics, and
  [nesting is bounded](#untrusted-input) so hostile input can't overflow the
  stack.
- **Beyond Rust:** [Python bindings](#python) and a
  [command-line viewer](#command-line).

## Contents

- [Install](#install)
- [Your first blob](#your-first-blob)
- [Values](#values)
- [Arrays without copying](#arrays-without-copying)
- [Large files](#large-files)
- [Secrets](#secrets)
- [Errors](#errors)
- [Untrusted input](#untrusted-input)
- [Command line](#command-line)
- [Python](#python)

## Install

blobfig isn't on crates.io yet. Depend on it from GitHub:

```toml
[dependencies]
blobfig = { git = "https://github.com/jonasrsv42/blobfig", branch = "main" }
```

`Cargo.lock` pins the commit you first built with, so upgrades are deliberate
(the API can still change): `cargo update -p blobfig` moves to the latest.
To pin a commit in `Cargo.toml` instead, use `rev = "<commit>"`.

It needs Rust 1.87 or newer. For the API docs, run
`cargo doc -p blobfig --all-features --open`.

## Your first blob

Build a `Value`, write it, parse it back, and read values by path:

```rust
use blobfig::{AccessError, Array, DType, File, NodeView, Value, parse, writer};

let config = Value::object(vec![
    ("version".into(), Value::Int(1)),
    ("labels".into(), Value::list(vec!["cat".into(), "dog".into()])),
    (
        "preprocessing".into(),
        Value::object(vec![(
            "mean".into(),
            Value::Array(Array::new(DType::F32, vec![3], vec![0; 12])),
        )]),
    ),
    ("model".into(), Value::File(File::from_bytes("application/x-tflite", vec![7; 2048]))),
]);
let bytes = writer::to_bytes(config).unwrap();

let parsed = parse(&bytes).unwrap();
assert_eq!(parsed.int("version").unwrap(), 1);

// Nested values by path
let mean = parsed.array("preprocessing/mean").unwrap();
assert_eq!((mean.dtype, mean.shape.as_slice()), (DType::F32, &[3][..]));

// A file is its mimetype and a slice of `bytes`
let model = parsed.file("model").unwrap();
assert_eq!(model.mimetype, "application/x-tflite");
assert_eq!(model.data, &bytes[bytes.len() - 2048..]);

// Objects and lists: `get`, then `as_object` or `as_list`
let labels = parsed.get("labels").unwrap().as_list().unwrap();
let labels: Vec<&str> = labels.iter().filter_map(|label| label.as_str()).collect();
assert_eq!(labels, ["cat", "dog"]);

// A wrong path or type is an error that says which
let error = parsed.int("labels").unwrap_err();
assert!(matches!(error, AccessError::TypeMismatch { .. }));
assert_eq!(error.to_string(), "type mismatch at 'labels': expected int, got List");
```

Things to notice:

- `parse` returns a `ValueView` that borrows from `bytes`, so `bytes` must
  outlive it. `.to_owned()` copies a view into a `Value` that stands alone.
- The accessors (`int`, `array`, `get`, …) come from the `NodeView` trait,
  which `Value` and `ValueView` share, so bring it into scope.
- Paths are keys joined by `/`, which is why a key can't contain one.
- Objects keep their entries in the order written, and can't hold a key
  twice.

## Values

| `Value` | Holds | Read with |
|---------|-------|-----------|
| `Bool`, `Int`, `Float` | `bool`, `i64`, `f64` | `bool(path)`, `int(path)`, `float(path)` |
| `String` | UTF-8 text | `string(path)` |
| `Array` | a `DType` (`U8` to `U64`, `I8` to `I64`, `F32`, `F64`), a shape, and the elements as little-endian bytes | `array(path)` |
| `File` | a mimetype and bytes | `file(path)` |
| `Object` | keys and values, in order | `get(path)`, then `as_object()` |
| `List` | values, in order | `get(path)`, then `as_list()` |
| `Secret` | any value, [redacted when printed](#secrets) | the accessor of the value inside |

Each accessor that takes a path returns a `Result`; on a value you already
hold, `as_bool()`, `as_int()`, … return an `Option`. There is no null: leave
the key out. For the full list, see the API docs.

## Arrays without copying

With the `ndarray` feature, arrays convert to and from
[ndarray](https://docs.rs/ndarray):

```toml
[dependencies]
blobfig = { git = "https://github.com/jonasrsv42/blobfig", branch = "main", features = ["ndarray"] }
```

`try_as_ndarray` gives a view into the blob, and `to_ndarray` a copy:

```rust
use blobfig::{Array, NodeView, Value, parse, writer};
use ndarray::array;

let weights = array![[1.0f32, 2.0], [3.0, 4.0]].into_dyn();
let bytes = writer::to_bytes(Value::Array(Array::from_ndarray(weights).unwrap())).unwrap();

let parsed = parse(&bytes).unwrap();
let array = parsed.as_array().unwrap();

// A view into `bytes` when the data is aligned for f32, else a copy
let sum: f32 = match array.try_as_ndarray::<f32>() {
    Ok(view) => view.sum(),
    Err(_) => array.to_ndarray::<f32>().unwrap().sum(),
};
assert_eq!(sum, 10.0);
```

The view needs the data aligned for its element type. Array and file data
start at multiples of `ALIGNMENT` (64 bytes) from the start of the blob, so
they are aligned whenever the buffer itself is, as a memory-mapped file
always is. A `Vec<u8>` from `std::fs::read` is usually aligned to 8 or 16
bytes, which is enough for every dtype but not guaranteed, hence the
fallback.

Both conversions check the element type, and fail with an `NdarrayError` if
it isn't the array's dtype. Without the feature, an array is still its
`dtype`, `shape` and `data` bytes.

## Large files

`writer::to_bytes` builds the blob in memory. For a model of a few
gigabytes, `writer::write` streams to any `io::Write`, and a file made with
`File::from_handle` is read as it is written instead of being loaded first.
A handle is a reader that knows its size:

```rust
use blobfig::{File, FileHandle, NodeView, Value, parse, writer};
use std::io::{self, Read};

// A reader, and how many bytes it will give
struct Source<R>(R, u64);

impl<R: Read> Read for Source<R> {
    fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
        self.0.read(buffer)
    }
}

impl<R: Read + Send> FileHandle for Source<R> {
    fn size(&self) -> u64 {
        self.1
    }
}

// Stands in for a `std::fs::File` and the length from its metadata
let model = Source(io::repeat(7).take(1 << 20), 1 << 20);
let config = Value::object(vec![(
    "model".into(),
    Value::File(File::from_handle("application/x-tflite", model)),
)]);

// Any `io::Write`, such as a `BufWriter` around a file
let mut blob = Vec::new();
writer::write(&mut blob, config).unwrap();

assert_eq!(parse(&blob).unwrap().file("model").unwrap().data.len(), 1 << 20);
```

A handle that ends before the size it gave fails the write with
`WriteErrorReason::FileTooShort`.

To read a large blob, memory-map it (with a crate such as
[memmap2](https://docs.rs/memmap2)) and pass the map to `parse`: only the
pages you touch are read from disk.

## Secrets

Wrap a value in `Value::secret(..)` and it prints as `<redacted>`, so logging
or debug-printing a whole config doesn't leak it. Code that asks for the
value still gets it:

```rust
use blobfig::{NodeView, Value, parse, writer};

let config = Value::object(vec![
    ("user".into(), Value::String("alice".into())),
    ("password".into(), Value::secret("hunter2")),
    // A secret object redacts everything in it
    ("db".into(), Value::secret(Value::object(vec![
        ("host".into(), Value::String("db.internal".into())),
        ("token".into(), Value::String("s3cr3t".into())),
    ]))),
]);
let bytes = writer::to_bytes(config).unwrap();
let parsed = parse(&bytes).unwrap();

println!("{parsed:?}");

// Reading sees through secrets, also on the way down a path
assert_eq!(parsed.string("password").unwrap(), "hunter2");
assert_eq!(parsed.string("db/token").unwrap(), "s3cr3t");
```

prints

```text
Object([("user", String("alice")), ("password", <redacted>), ("db", <redacted>)])
```

> **Redaction, not encryption.** A secret is stored as plain bytes in the blob
> and in memory. This prevents *accidental* disclosure through printing and
> logging; anyone who reads the value or the file sees it.

When an owned `Value::secret` is dropped, blobfig zeroes the bytes it owns,
spare capacity included. Copies the wipe can't reach stay where they are:

- what `parse` reads, which borrows your buffer, so wiping it is up to you;
- the output of `writer`, including what a growing `Vec` leaves behind when
  it reallocates;
- a payload you move out with `String::try_from`, `Object::try_from` and the
  like, which is yours from then on. Children of an object or list you move
  out are neither redacted nor wiped unless they are secrets themselves;
- keys inside a secret that a write error names in its path;
- copies made before the value was marked secret, temporaries on the stack
  (such as the buffer the writer streams a file handle through), and a file
  handle's bytes;
- a secret that is never dropped, as after `mem::forget` or an abort.

## Errors

`parse` fails with a `ParseError`. Printed, it says what went wrong and
where. For code, `reason()` is an enum to match on, and the reason's
`class()` sorts every reason into a few kinds by what a caller can do:

```rust
use blobfig::{ParseErrorClass, ParseErrorReason, Value, parse, writer};

let blob = writer::to_bytes(Value::String("hello".into())).unwrap();
let cut_off = &blob[..blob.len() - 2];
let json = br#"{"version": 1}"#;

for input in [cut_off, &json[..]] {
    let error = parse(input).unwrap_err();
    let advice = match error.reason().class() {
        ParseErrorClass::NotBlobfig => "is this the right file?",
        ParseErrorClass::Unsupported => "upgrade blobfig",
        ParseErrorClass::Corrupt => "fetch it again",
        _ => "refuse it",
    };
    println!("{error}: {advice}");
}

let error = parse(cut_off).unwrap_err();
assert_eq!(error.reason(), ParseErrorReason::Truncated);
assert_eq!(error.reason().code(), "truncated");
assert_eq!(error.offset(), 21);
```

prints

```text
unexpected end of input at offset 21: fetch it again
not a blobfig file (bad magic bytes) at offset 0: is this the right file?
```

Things to notice:

- `offset()` is the position in the input where parsing failed. Here that is
  where the string's five bytes start, since only three of them are there.
- `code()` is a stable name for the reason, for logs and other languages.
- `ParseErrorReason` and `ParseErrorClass` are `#[non_exhaustive]`, so a
  `match` needs a `_` arm, and reasons that carry data are matched with
  `{ .. }`, as in `ParseErrorReason::BadTag { .. }`.

There are two more error types: `AccessError` when reading a value by path
(not found, or another type), and `WriteError` when writing. A
`WriteError`'s `reason()` is a `WriteErrorReason` (I/O, or a value the
format can't hold), and its `path()` is the value it failed at, which its
message starts with:

```text
at "model/layers/3": array data is 12 bytes, but F32 with shape [4] needs 16 bytes
```

The API docs list every reason and variant.

## Untrusted input

`parse` checks everything it reads and returns an error instead of
panicking: lengths that run past the input, bytes left over after the value,
invalid UTF-8, array data that doesn't match its dtype and shape, and
duplicate keys are all refused.

Each level of nesting uses stack, so values nest at most `MAX_DEPTH` (12)
deep. Deeper input fails with `ParseErrorReason::TooDeep` instead of
overflowing the stack:

```rust
use blobfig::{
    HEADER_SIZE, MAX_DEPTH, ParseErrorClass, ParseErrorReason, Value, WriteErrorReason, parse,
    writer,
};

// A valid header, then a thousand lists of one item, each inside the last
let mut hostile = writer::to_bytes(Value::Int(1)).unwrap();
hostile.truncate(HEADER_SIZE);
for _ in 0..1000 {
    hostile.push(0x08); // the tag of a list
    hostile.extend(1u32.to_le_bytes()); // its item count
}

let error = parse(&hostile).unwrap_err();
assert_eq!(error.reason(), ParseErrorReason::TooDeep);
assert_eq!(error.reason().class(), ParseErrorClass::Limit);

// The writer has the same limit
let mut deep = Value::Int(1);
for _ in 0..MAX_DEPTH {
    deep = Value::list(vec![deep]);
}
let error = writer::to_bytes(deep).unwrap_err();
assert!(matches!(error.reason(), WriteErrorReason::TooDeep));
```

The limit is sized so the parser fits a 32 KiB stack, such as an ESP32
task's. The writer refuses whatever the parser would (a key with `/`, a
duplicate key, mismatched array data, too much nesting), so what it writes
parses back.

Strings, arrays and files are never copied, so a length field that claims
gigabytes fails as `Truncated` without allocating them.

## Command line

`blobfig-cli` installs a `blobfig` command that prints a blob, with secrets
redacted, or says why it doesn't parse:

```text
$ blobfig model.blobfig
sample_rate: 16000
api_key: <redacted>
mean: <array F32 [3], 12 B>
model: <file application/x-tflite, 2.0 KiB>
```

Install it from a checkout with `./blobfig-cli/install.sh` (into
`~/.local/bin`) or `cargo install --path blobfig-cli`.

## Python

[blobfigy](https://github.com/jonasrsv42/blobfigy) reads and writes blobs
from Python: dicts and lists, numpy arrays as zero-copy views of a
memory-mapped blob, files and secrets.

## License

MIT
