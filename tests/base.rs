//! Integration tests for blobfig
//!
//! These tests demonstrate the main use cases for the blobfig format.

use blobfig::{Array, DType, File, FileHandle, HEADER_SIZE, MAGIC, VERSION, Value, parse, writer};
use std::io::{self, Read};

// =============================================================================
// Basic value roundtrips
// =============================================================================

#[test]
fn roundtrip_primitives() {
    // Bool
    let bytes = writer::to_bytes(Value::Bool(true)).unwrap();
    assert_eq!(parse(&bytes).unwrap().as_bool(), Some(true));

    let bytes = writer::to_bytes(Value::Bool(false)).unwrap();
    assert_eq!(parse(&bytes).unwrap().as_bool(), Some(false));

    // Int
    let bytes = writer::to_bytes(Value::Int(i64::MIN)).unwrap();
    assert_eq!(parse(&bytes).unwrap().as_int(), Some(i64::MIN));

    let bytes = writer::to_bytes(Value::Int(i64::MAX)).unwrap();
    assert_eq!(parse(&bytes).unwrap().as_int(), Some(i64::MAX));

    // Float
    let bytes = writer::to_bytes(Value::Float(std::f64::consts::PI)).unwrap();
    let parsed = parse(&bytes).unwrap().as_float().unwrap();
    assert!((parsed - std::f64::consts::PI).abs() < 1e-15);

    // String
    let bytes = writer::to_bytes(Value::String("hello 世界".into())).unwrap();
    assert_eq!(parse(&bytes).unwrap().as_str(), Some("hello 世界"));
}

// =============================================================================
// Arrays (typed tensors)
// =============================================================================

#[test]
fn roundtrip_array_1d() {
    // Create a 1D array of f32 values
    let data: Vec<f32> = vec![1.0, 2.0, 3.0, 4.0];
    let bytes: Vec<u8> = data.iter().flat_map(|f| f.to_le_bytes()).collect();

    let array = Array::new(DType::F32, vec![4], bytes);
    let value = Value::Array(array);

    let encoded = writer::to_bytes(value).unwrap();
    let parsed = parse(&encoded).unwrap();

    let arr = parsed.as_array().unwrap();
    assert_eq!(arr.dtype, DType::F32);
    assert_eq!(arr.shape, vec![4]);
    assert_eq!(arr.data.len(), 16); // 4 * 4 bytes
}

#[test]
fn roundtrip_array_2d() {
    // Create a 2x3 matrix of i32 values
    let data: Vec<i32> = vec![1, 2, 3, 4, 5, 6];
    let bytes: Vec<u8> = data.iter().flat_map(|i| i.to_le_bytes()).collect();

    let array = Array::new(DType::I32, vec![2, 3], bytes);
    let value = Value::Array(array);

    let encoded = writer::to_bytes(value).unwrap();
    let parsed = parse(&encoded).unwrap();

    let arr = parsed.as_array().unwrap();
    assert_eq!(arr.dtype, DType::I32);
    assert_eq!(arr.shape, vec![2, 3]);

    // Verify data integrity
    let mut cursor = arr.data;
    for expected in &data {
        let val = i32::from_le_bytes([cursor[0], cursor[1], cursor[2], cursor[3]]);
        assert_eq!(val, *expected);
        cursor = &cursor[4..];
    }
}

// =============================================================================
// Files (embedded blobs)
// =============================================================================

#[test]
fn roundtrip_file_bytes() {
    let content = b"This is a test file content";
    let file = File::from_bytes("text/plain", content.to_vec());
    let value = Value::File(file);

    let encoded = writer::to_bytes(value).unwrap();
    let parsed = parse(&encoded).unwrap();

    let f = parsed.as_file().unwrap();
    assert_eq!(f.mimetype, "text/plain");
    assert_eq!(f.data, content);
}

#[test]
fn roundtrip_file_binary() {
    // Simulate a binary file (like a model)
    let content: Vec<u8> = (0..256).map(|i| i as u8).collect();
    let file = File::from_bytes("application/x-tflite", content.clone());
    let value = Value::File(file);

    let encoded = writer::to_bytes(value).unwrap();
    let parsed = parse(&encoded).unwrap();

    let f = parsed.as_file().unwrap();
    assert_eq!(f.mimetype, "application/x-tflite");
    assert_eq!(f.data, content.as_slice());
}

/// Custom FileHandle for testing streaming writes
struct TestFileHandle {
    data: Vec<u8>,
    position: usize,
}

impl TestFileHandle {
    fn new(data: Vec<u8>) -> Self {
        Self { data, position: 0 }
    }
}

impl Read for TestFileHandle {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        let remaining = &self.data[self.position..];
        let to_read = buf.len().min(remaining.len());
        buf[..to_read].copy_from_slice(&remaining[..to_read]);
        self.position += to_read;
        Ok(to_read)
    }
}

impl FileHandle for TestFileHandle {
    fn size(&self) -> u64 {
        self.data.len() as u64
    }
}

#[test]
fn roundtrip_file_streaming() {
    let content: Vec<u8> = (0..1000).map(|i| (i % 256) as u8).collect();
    let handle = TestFileHandle::new(content.clone());
    let file = File::from_handle("application/octet-stream", handle);
    let value = Value::File(file);

    let encoded = writer::to_bytes(value).unwrap();
    let parsed = parse(&encoded).unwrap();

    let f = parsed.as_file().unwrap();
    assert_eq!(f.mimetype, "application/octet-stream");
    assert_eq!(f.data, content.as_slice());
}

// =============================================================================
// Objects and nested structures
// =============================================================================

#[test]
fn roundtrip_object() {
    let value = Value::Object(vec![
        ("name".into(), Value::String("test-config".into())),
        ("version".into(), Value::Int(1)),
        ("enabled".into(), Value::Bool(true)),
        ("threshold".into(), Value::Float(0.95)),
    ]);

    let encoded = writer::to_bytes(value).unwrap();
    let parsed = parse(&encoded).unwrap();

    let obj = parsed.as_object().unwrap();
    assert_eq!(obj.len(), 4);
    assert_eq!(obj[0].0, "name");
    assert_eq!(obj[0].1.as_str(), Some("test-config"));
    assert_eq!(obj[1].1.as_int(), Some(1));
    assert_eq!(obj[2].1.as_bool(), Some(true));
}

#[test]
fn roundtrip_nested_object() {
    let value = Value::Object(vec![
        (
            "model".into(),
            Value::Object(vec![
                ("name".into(), Value::String("bert-base".into())),
                ("layers".into(), Value::Int(12)),
            ]),
        ),
        (
            "training".into(),
            Value::Object(vec![
                ("epochs".into(), Value::Int(100)),
                ("learning_rate".into(), Value::Float(0.001)),
            ]),
        ),
    ]);

    let encoded = writer::to_bytes(value).unwrap();
    let parsed = parse(&encoded).unwrap();

    // Test path accessor
    assert_eq!(
        parsed.get("model/name").unwrap().as_str(),
        Some("bert-base")
    );
    assert_eq!(parsed.get("model/layers").unwrap().as_int(), Some(12));
    assert_eq!(parsed.get("training/epochs").unwrap().as_int(), Some(100));
}

#[test]
fn roundtrip_list() {
    let value = Value::List(vec![
        Value::Int(1),
        Value::Int(2),
        Value::Int(3),
        Value::String("four".into()),
        Value::Bool(true),
    ]);

    let encoded = writer::to_bytes(value).unwrap();
    let parsed = parse(&encoded).unwrap();

    let list = parsed.as_list().unwrap();
    assert_eq!(list.len(), 5);
    assert_eq!(list[0].as_int(), Some(1));
    assert_eq!(list[3].as_str(), Some("four"));
    assert_eq!(list[4].as_bool(), Some(true));
}

// =============================================================================
// ML config use case
// =============================================================================

#[test]
fn ml_config_use_case() {
    // Simulate a real ML configuration with model weights and preprocessing stats
    let model_weights: Vec<u8> = vec![0xDE, 0xAD, 0xBE, 0xEF]; // Fake model bytes
    let mean_values: Vec<f32> = vec![0.485, 0.456, 0.406];
    let std_values: Vec<f32> = vec![0.229, 0.224, 0.225];

    let config = Value::Object(vec![
        ("version".into(), Value::Int(1)),
        (
            "model_type".into(),
            Value::String("image-classifier".into()),
        ),
        (
            "model".into(),
            Value::File(File::from_bytes(
                "application/x-tflite",
                model_weights.clone(),
            )),
        ),
        (
            "preprocessing".into(),
            Value::Object(vec![
                (
                    "mean".into(),
                    Value::Array(Array::new(
                        DType::F32,
                        vec![3],
                        mean_values.iter().flat_map(|f| f.to_le_bytes()).collect(),
                    )),
                ),
                (
                    "std".into(),
                    Value::Array(Array::new(
                        DType::F32,
                        vec![3],
                        std_values.iter().flat_map(|f| f.to_le_bytes()).collect(),
                    )),
                ),
            ]),
        ),
        (
            "labels".into(),
            Value::List(vec![
                Value::String("cat".into()),
                Value::String("dog".into()),
                Value::String("bird".into()),
            ]),
        ),
    ]);

    let encoded = writer::to_bytes(config).unwrap();
    let parsed = parse(&encoded).unwrap();

    // Verify structure
    assert_eq!(parsed.get("version").unwrap().as_int(), Some(1));
    assert_eq!(
        parsed.get("model_type").unwrap().as_str(),
        Some("image-classifier")
    );

    // Verify model blob
    let model = parsed.get("model").unwrap().as_file().unwrap();
    assert_eq!(model.mimetype, "application/x-tflite");
    assert_eq!(model.data, model_weights.as_slice());

    // Verify preprocessing arrays
    let mean = parsed
        .get("preprocessing/mean")
        .unwrap()
        .as_array()
        .unwrap();
    assert_eq!(mean.dtype, DType::F32);
    assert_eq!(mean.shape, vec![3]);

    // Verify labels
    let labels = parsed.get("labels").unwrap().as_list().unwrap();
    assert_eq!(labels.len(), 3);
    assert_eq!(labels[0].as_str(), Some("cat"));
}

// =============================================================================
// Zero-copy verification
// =============================================================================

#[test]
fn verify_zero_copy_parsing() {
    let content = b"This content should be zero-copy referenced";
    let value = Value::Object(vec![
        (
            "data".into(),
            Value::String(String::from_utf8_lossy(content).into()),
        ),
        (
            "file".into(),
            Value::File(File::from_bytes("text/plain", content.to_vec())),
        ),
    ]);

    let encoded = writer::to_bytes(value).unwrap();
    let parsed = parse(&encoded).unwrap();

    // The parsed string and file data should point into the encoded buffer
    let s = parsed.get("data").unwrap().as_str().unwrap();
    let f = parsed.get("file").unwrap().as_file().unwrap();

    // Verify pointers are within the encoded buffer range
    let encoded_start = encoded.as_ptr() as usize;
    let encoded_end = encoded_start + encoded.len();

    let str_ptr = s.as_ptr() as usize;
    assert!(
        str_ptr >= encoded_start && str_ptr < encoded_end,
        "String should be zero-copy"
    );

    let file_ptr = f.data.as_ptr() as usize;
    assert!(
        file_ptr >= encoded_start && file_ptr < encoded_end,
        "File data should be zero-copy"
    );
}

// =============================================================================
// Header verification
// =============================================================================

#[test]
fn verify_header_format() {
    let value = Value::Bool(true);
    let encoded = writer::to_bytes(value).unwrap();

    // Check magic bytes
    assert_eq!(&encoded[0..8], MAGIC);

    // Check version
    let version = u32::from_le_bytes([encoded[8], encoded[9], encoded[10], encoded[11]]);
    assert_eq!(version, VERSION);

    // Check total header size
    assert!(encoded.len() >= HEADER_SIZE);
}

// =============================================================================
// Edge cases
// =============================================================================

#[test]
fn empty_string() {
    let value = Value::String("".into());
    let encoded = writer::to_bytes(value).unwrap();
    let parsed = parse(&encoded).unwrap();
    assert_eq!(parsed.as_str(), Some(""));
}

#[test]
fn empty_list() {
    let value = Value::List(vec![]);
    let encoded = writer::to_bytes(value).unwrap();
    let parsed = parse(&encoded).unwrap();
    assert_eq!(parsed.as_list().unwrap().len(), 0);
}

#[test]
fn empty_object() {
    let value = Value::Object(vec![]);
    let encoded = writer::to_bytes(value).unwrap();
    let parsed = parse(&encoded).unwrap();
    assert_eq!(parsed.as_object().unwrap().len(), 0);
}

#[test]
fn empty_array() {
    let array = Array::new(DType::F32, vec![0], vec![]);
    let value = Value::Array(array);
    let encoded = writer::to_bytes(value).unwrap();
    let parsed = parse(&encoded).unwrap();

    let arr = parsed.as_array().unwrap();
    assert_eq!(arr.shape, vec![0]);
    assert_eq!(arr.data.len(), 0);
}

#[test]
fn empty_file() {
    let file = File::from_bytes("application/octet-stream", vec![]);
    let value = Value::File(file);
    let encoded = writer::to_bytes(value).unwrap();
    let parsed = parse(&encoded).unwrap();

    let f = parsed.as_file().unwrap();
    assert_eq!(f.data.len(), 0);
}

#[test]
fn deeply_nested() {
    let value = Value::Object(vec![(
        "a".into(),
        Value::Object(vec![(
            "b".into(),
            Value::Object(vec![(
                "c".into(),
                Value::Object(vec![("d".into(), Value::Int(42))]),
            )]),
        )]),
    )]);

    let encoded = writer::to_bytes(value).unwrap();
    let parsed = parse(&encoded).unwrap();

    assert_eq!(parsed.get("a/b/c/d").unwrap().as_int(), Some(42));
}
