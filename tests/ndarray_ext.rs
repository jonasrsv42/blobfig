//! Integration tests for ndarray support
//!
//! These tests demonstrate using blobfig with ndarray for ML workflows.

#![cfg(feature = "ndarray")]

use blobfig::{Array, File, Value, parse, writer};
use ndarray::{ArrayD, IxDyn, array, s};

// =============================================================================
// Basic ndarray roundtrips
// =============================================================================

#[test]
fn roundtrip_ndarray_1d() {
    let arr = array![1.0f32, 2.0, 3.0, 4.0, 5.0].into_dyn();
    let expected = arr.clone();
    let value = Value::Array(Array::from_ndarray(arr).unwrap());

    let bytes = writer::to_bytes(value).unwrap();
    let parsed = parse(&bytes).unwrap();

    let back: ArrayD<f32> = parsed.as_array().unwrap().to_ndarray().unwrap();
    assert_eq!(expected, back);
}

#[test]
fn roundtrip_ndarray_2d_matrix() {
    let matrix = array![[1.0f64, 2.0, 3.0], [4.0, 5.0, 6.0]].into_dyn();
    let expected = matrix.clone();
    let value = Value::Array(Array::from_ndarray(matrix).unwrap());

    let bytes = writer::to_bytes(value).unwrap();
    let parsed = parse(&bytes).unwrap();

    let back: ArrayD<f64> = parsed.as_array().unwrap().to_ndarray().unwrap();
    assert_eq!(expected, back);
}

#[test]
fn roundtrip_ndarray_3d_tensor() {
    let tensor = ArrayD::<i32>::from_shape_fn(IxDyn(&[2, 3, 4]), |idx| {
        (idx[0] * 12 + idx[1] * 4 + idx[2]) as i32
    });
    let expected = tensor.clone();
    let value = Value::Array(Array::from_ndarray(tensor).unwrap());

    let bytes = writer::to_bytes(value).unwrap();
    let parsed = parse(&bytes).unwrap();

    let back: ArrayD<i32> = parsed.as_array().unwrap().to_ndarray().unwrap();
    assert_eq!(expected, back);
}

// =============================================================================
// ML preprocessing config
// =============================================================================

#[test]
fn image_preprocessing_config() {
    let mean = array![0.485f32, 0.456, 0.406].into_dyn();
    let std = array![0.229f32, 0.224, 0.225].into_dyn();

    let config = Value::Object(vec![
        (
            "input_size".into(),
            Value::List(vec![Value::Int(224), Value::Int(224)]),
        ),
        ("channels".into(), Value::Int(3)),
        (
            "normalize".into(),
            Value::Object(vec![
                (
                    "mean".into(),
                    Value::Array(Array::from_ndarray(mean).unwrap()),
                ),
                (
                    "std".into(),
                    Value::Array(Array::from_ndarray(std).unwrap()),
                ),
            ]),
        ),
    ]);

    let bytes = writer::to_bytes(config).unwrap();
    let parsed = parse(&bytes).unwrap();

    let mean_back: ArrayD<f32> = parsed
        .get("normalize/mean")
        .unwrap()
        .as_array()
        .unwrap()
        .to_ndarray()
        .unwrap();

    let std_back: ArrayD<f32> = parsed
        .get("normalize/std")
        .unwrap()
        .as_array()
        .unwrap()
        .to_ndarray()
        .unwrap();

    assert!((mean_back[[0]] - 0.485).abs() < 1e-6);
    assert!((std_back[[2]] - 0.225).abs() < 1e-6);
}

// =============================================================================
// Model weights simulation
// =============================================================================

#[test]
fn layer_weights_config() {
    let weights = ArrayD::<f32>::from_shape_fn(IxDyn(&[64, 128]), |idx| {
        ((idx[0] * 128 + idx[1]) as f32) * 0.01
    });
    let bias = ArrayD::<f32>::zeros(IxDyn(&[64]));
    let expected_weights = weights.clone();

    let layer = Value::Object(vec![
        ("name".into(), Value::String("dense_1".into())),
        (
            "weights".into(),
            Value::Array(Array::from_ndarray(weights).unwrap()),
        ),
        (
            "bias".into(),
            Value::Array(Array::from_ndarray(bias).unwrap()),
        ),
        ("activation".into(), Value::String("relu".into())),
    ]);

    let bytes = writer::to_bytes(layer).unwrap();
    let parsed = parse(&bytes).unwrap();

    let weights_back: ArrayD<f32> = parsed
        .get("weights")
        .unwrap()
        .as_array()
        .unwrap()
        .to_ndarray()
        .unwrap();

    assert_eq!(weights_back.shape(), &[64, 128]);
    assert_eq!(expected_weights, weights_back);
}

// =============================================================================
// Complete ML artifact
// =============================================================================

#[test]
fn complete_ml_artifact() {
    let embedding = ArrayD::<f32>::from_shape_fn(IxDyn(&[1000, 128]), |idx| {
        ((idx[0] * 128 + idx[1]) % 256) as f32 / 256.0
    });
    let mean = array![0.5f32, 0.5, 0.5].into_dyn();
    let std = array![0.25f32, 0.25, 0.25].into_dyn();
    let model_bytes: Vec<u8> = (0..1024).map(|i| (i % 256) as u8).collect();

    let expected_mean = mean.clone();
    let expected_embedding = embedding.clone();

    let artifact = Value::Object(vec![
        ("version".into(), Value::Int(1)),
        (
            "model_type".into(),
            Value::String("image-classifier".into()),
        ),
        (
            "model".into(),
            Value::File(File::from_bytes("application/x-tflite", model_bytes)),
        ),
        (
            "preprocessing".into(),
            Value::Object(vec![
                (
                    "mean".into(),
                    Value::Array(Array::from_ndarray(mean).unwrap()),
                ),
                (
                    "std".into(),
                    Value::Array(Array::from_ndarray(std).unwrap()),
                ),
                (
                    "input_shape".into(),
                    Value::List(vec![Value::Int(224), Value::Int(224), Value::Int(3)]),
                ),
            ]),
        ),
        (
            "embedding".into(),
            Value::Array(Array::from_ndarray(embedding).unwrap()),
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

    let bytes = writer::to_bytes(artifact).unwrap();
    let parsed = parse(&bytes).unwrap();

    assert_eq!(parsed.get("version").unwrap().as_int(), Some(1));
    assert_eq!(
        parsed.get("model_type").unwrap().as_str(),
        Some("image-classifier")
    );

    let model = parsed.get("model").unwrap().as_file().unwrap();
    assert_eq!(model.mimetype, "application/x-tflite");
    assert_eq!(model.data.len(), 1024);

    let mean_back: ArrayD<f32> = parsed
        .get("preprocessing/mean")
        .unwrap()
        .as_array()
        .unwrap()
        .to_ndarray()
        .unwrap();
    assert_eq!(expected_mean, mean_back);

    let emb_back: ArrayD<f32> = parsed
        .get("embedding")
        .unwrap()
        .as_array()
        .unwrap()
        .to_ndarray()
        .unwrap();
    assert_eq!(emb_back.shape(), &[1000, 128]);
    assert_eq!(expected_embedding, emb_back);

    let labels = parsed.get("labels").unwrap().as_list().unwrap();
    assert_eq!(labels[0].as_str(), Some("cat"));
}

// =============================================================================
// Batch data
// =============================================================================

#[test]
fn batch_of_samples() {
    let batch_size = 32;
    let features = 64;

    let x = ArrayD::<f32>::from_shape_fn(IxDyn(&[batch_size, features]), |idx| {
        (idx[0] * features + idx[1]) as f32
    });
    let y = ArrayD::<i64>::from_shape_fn(IxDyn(&[batch_size]), |idx| (idx[0] % 10) as i64);
    let expected_x = x.clone();
    let expected_y = y.clone();

    let batch = Value::Object(vec![
        ("x".into(), Value::Array(Array::from_ndarray(x).unwrap())),
        ("y".into(), Value::Array(Array::from_ndarray(y).unwrap())),
        ("batch_size".into(), Value::Int(batch_size as i64)),
    ]);

    let bytes = writer::to_bytes(batch).unwrap();
    let parsed = parse(&bytes).unwrap();

    let x_back: ArrayD<f32> = parsed
        .get("x")
        .unwrap()
        .as_array()
        .unwrap()
        .to_ndarray()
        .unwrap();
    let y_back: ArrayD<i64> = parsed
        .get("y")
        .unwrap()
        .as_array()
        .unwrap()
        .to_ndarray()
        .unwrap();

    assert_eq!(expected_x, x_back);
    assert_eq!(expected_y, y_back);
}

// =============================================================================
// Multiple array dtypes
// =============================================================================

#[test]
fn mixed_dtype_arrays() {
    let u8_data = array![255u8, 128, 64, 32, 16, 8, 4, 2, 1, 0].into_dyn();
    let i16_data = array![-1000i16, -100, -10, 0, 10, 100, 1000].into_dyn();
    let f64_data = array![
        std::f64::consts::PI,
        std::f64::consts::E,
        std::f64::consts::TAU
    ]
    .into_dyn();

    let expected_u8 = u8_data.clone();
    let expected_i16 = i16_data.clone();
    let expected_f64 = f64_data.clone();

    let config = Value::Object(vec![
        (
            "quantized_weights".into(),
            Value::Array(Array::from_ndarray(u8_data).unwrap()),
        ),
        (
            "offsets".into(),
            Value::Array(Array::from_ndarray(i16_data).unwrap()),
        ),
        (
            "constants".into(),
            Value::Array(Array::from_ndarray(f64_data).unwrap()),
        ),
    ]);

    let bytes = writer::to_bytes(config).unwrap();
    let parsed = parse(&bytes).unwrap();

    let u8_back: ArrayD<u8> = parsed
        .get("quantized_weights")
        .unwrap()
        .as_array()
        .unwrap()
        .to_ndarray()
        .unwrap();
    let i16_back: ArrayD<i16> = parsed
        .get("offsets")
        .unwrap()
        .as_array()
        .unwrap()
        .to_ndarray()
        .unwrap();
    let f64_back: ArrayD<f64> = parsed
        .get("constants")
        .unwrap()
        .as_array()
        .unwrap()
        .to_ndarray()
        .unwrap();

    assert_eq!(expected_u8, u8_back);
    assert_eq!(expected_i16, i16_back);
    assert_eq!(expected_f64, f64_back);
}

// =============================================================================
// Tokenizer vocab as embedding lookup
// =============================================================================

#[test]
fn tokenizer_with_embeddings() {
    let vocab_size = 256;
    let embed_dim = 32;

    let embeddings = ArrayD::<f32>::from_shape_fn(IxDyn(&[vocab_size, embed_dim]), |idx| {
        ((idx[0] + idx[1]) % 100) as f32 / 100.0
    });

    let config = Value::Object(vec![
        ("vocab_size".into(), Value::Int(vocab_size as i64)),
        ("embed_dim".into(), Value::Int(embed_dim as i64)),
        (
            "embeddings".into(),
            Value::Array(Array::from_ndarray(embeddings).unwrap()),
        ),
        (
            "special_tokens".into(),
            Value::Object(vec![
                ("pad".into(), Value::Int(0)),
                ("unk".into(), Value::Int(1)),
                ("bos".into(), Value::Int(2)),
                ("eos".into(), Value::Int(3)),
            ]),
        ),
    ]);

    let bytes = writer::to_bytes(config).unwrap();
    let parsed = parse(&bytes).unwrap();

    let emb: ArrayD<f32> = parsed
        .get("embeddings")
        .unwrap()
        .as_array()
        .unwrap()
        .to_ndarray()
        .unwrap();
    assert_eq!(emb.shape(), &[256, 32]);

    let token_42_embedding = emb.slice(s![42, ..]);
    assert_eq!(token_42_embedding.len(), 32);
}
