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

## Features

- `ndarray` - ndarray conversion support
- `areamy` - areamy error integration
