# blobfig

Binary configuration format with zero-copy parsing. Bundle config, typed arrays, and file blobs into a single artifact.

## Usage

```rust
use blobfig::{Value, File, Array, DType, writer, parse};

// Build an ML artifact
let config = Value::Object(vec![
    ("version".into(), Value::Int(1)),
    ("model".into(), Value::File(
        File::from_bytes("application/x-tflite", model_bytes)
    )),
    ("mean".into(), Value::Array(
        Array::new(DType::F32, vec![3], mean_bytes)
    )),
]);

// Serialize
let bytes = writer::to_bytes(config).unwrap();

// Parse (zero-copy from mmap'd file)
let parsed = parse(&bytes).unwrap();
let version = parsed.get("version").unwrap().as_int();
let model = parsed.get("model").unwrap().as_file().unwrap();

// Nested access with path
let mean = parsed.get("preprocessing/mean").unwrap().as_array();
```

## With ndarray

```rust
use blobfig::{Value, Array, writer, parse};
use ndarray::array;

let weights = array![[1.0f32, 2.0], [3.0, 4.0]].into_dyn();
let config = Value::Array(Array::from_ndarray(weights).unwrap());

let bytes = writer::to_bytes(config).unwrap();
let parsed = parse(&bytes).unwrap();
let back: ndarray::ArrayD<f32> = parsed.as_array().unwrap().to_ndarray().unwrap();
```

## Secrets

Wrap any value in `Value::secret(..)` to mark it as secret. Secrets print as
`<redacted>`, so logging or debug-printing a whole blobfig never leaks them —
but programmatic access (`get`, the typed accessors) sees through transparently.

```rust
use blobfig::{Value, writer, parse};

let config = Value::Object(vec![
    ("user".into(), Value::String("alice".into())),
    ("password".into(), Value::secret("hunter2")),
    // A whole sub-object can be secret; the entire subtree is redacted.
    ("db".into(), Value::secret(Value::Object(vec![
        ("host".into(), Value::String("db.internal".into())),
        ("token".into(), Value::String("s3cr3t".into())),
    ]))),
]);

let bytes = writer::to_bytes(config).unwrap();
let parsed = parse(&bytes).unwrap();

// Printing redacts — no secret bytes appear, even with {:#?}:
//   Object([("user", String("alice")), ("password", <redacted>), ("db", <redacted>)])
println!("{:?}", parsed);

// Explicit access still returns the value (you asked for it):
assert_eq!(parsed.string("password").unwrap(), "hunter2");
assert_eq!(parsed.string("db/token").unwrap(), "s3cr3t"); // descends through the secret
```

> **Redaction, not encryption.** Secret bytes are stored in plaintext in the
> blob and live in plaintext in memory — this only prevents *accidental*
> disclosure via printing/logging. It is not protection at rest, and anyone who
> explicitly reads the value (or the raw file) sees it.

## Features

- `ndarray` - ndarray conversion support
- `areamy` - areamy error integration
