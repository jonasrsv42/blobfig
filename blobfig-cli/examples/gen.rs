use blobfig::{File, Value, writer};
fn main() {
    let cfg = Value::object(vec![
        ("version".into(), Value::Int(1)),
        ("user".into(), Value::String("alice".into())),
        ("password".into(), Value::secret("hunter2")),
        (
            "model".into(),
            Value::File(File::from_bytes("application/x-tflite", vec![0u8; 2048])),
        ),
        (
            "db".into(),
            Value::secret(Value::object(vec![
                ("host".into(), Value::String("db.internal".into())),
                ("token".into(), Value::String("s3cr3t".into())),
            ])),
        ),
    ]);
    let bytes = writer::to_bytes(cfg).unwrap();
    std::fs::write("/tmp/sample.blobfig", bytes).unwrap();
}
