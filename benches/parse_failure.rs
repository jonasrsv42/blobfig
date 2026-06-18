//! Benchmarks isolating the cost of the *failure* path when a blob contains a
//! large binary payload.
//!
//! Hypothesis under test: parsing itself is zero-copy and cheap regardless of
//! payload size, and constructing an error is also cheap. But *rendering* a
//! parse error (Display) walks the entire input buffer and allocates a full
//! `String` copy of it (via `CodeLoc::readable_position` / `context_lines` ->
//! `String::from_utf8_lossy(whole_buffer)`), so the failure path becomes
//! O(blob size) the moment anyone formats the error — which is exactly what a
//! caller does when logging "why did parsing fail".
//!
//! Run with: `cargo bench --bench parse_failure`

use blobfig::{HEADER_SIZE, MAGIC, VERSION, parse};
use criterion::{BenchmarkId, Criterion, Throughput, criterion_group, criterion_main};
use std::hint::black_box;

const TAG_FILE: u8 = 0x06;
const TAG_LIST: u8 = 0x08;

fn header() -> Vec<u8> {
    let mut h = Vec::with_capacity(HEADER_SIZE);
    h.extend_from_slice(MAGIC);
    h.extend_from_slice(&VERSION.to_le_bytes());
    h.extend_from_slice(&0u32.to_le_bytes()); // flags
    debug_assert_eq!(h.len(), HEADER_SIZE);
    h
}

/// Encode a File value body: [u16 mimetype_len][mimetype][u64 data_size][data]
fn push_file(buf: &mut Vec<u8>, mimetype: &str, data: &[u8]) {
    buf.push(TAG_FILE);
    buf.extend_from_slice(&(mimetype.len() as u16).to_le_bytes());
    buf.extend_from_slice(mimetype.as_bytes());
    buf.extend_from_slice(&(data.len() as u64).to_le_bytes());
    buf.extend_from_slice(data);
}

/// A valid blob containing a single big File. Parses successfully (zero-copy).
fn valid_blob(payload: usize) -> Vec<u8> {
    let data = vec![0xABu8; payload];
    let mut buf = header();
    push_file(&mut buf, "application/x-tflite", &data);
    buf
}

/// A blob that contains the full big File, followed by an invalid trailing
/// value. The big binary parses fine, then parsing fails *after* it — so the
/// error's location buffer still spans the entire payload.
///
/// Layout: List(count=2) { File(big), <invalid tag 0xFF> }
fn failing_blob(payload: usize) -> Vec<u8> {
    let data = vec![0xABu8; payload];
    let mut buf = header();
    buf.push(TAG_LIST);
    buf.extend_from_slice(&2u32.to_le_bytes()); // 2 items
    push_file(&mut buf, "application/x-tflite", &data);
    buf.push(0xFF); // invalid value tag -> SyntaxError after the binary
    buf
}

fn bench(c: &mut Criterion) {
    let sizes = [1 << 20, 8 << 20, 64 << 20]; // 1 MiB, 8 MiB, 64 MiB

    // Baseline: successful parse of a big binary. Should be ~constant time.
    let mut ok = c.benchmark_group("parse_ok");
    for &size in &sizes {
        let bytes = valid_blob(size);
        ok.throughput(Throughput::Bytes(bytes.len() as u64));
        ok.bench_with_input(BenchmarkId::from_parameter(size), &bytes, |b, bytes| {
            b.iter(|| {
                let v = parse(black_box(bytes)).unwrap();
                black_box(v);
            });
        });
    }
    ok.finish();

    // Failure path, error NOT rendered: just observe that parsing failed.
    let mut err = c.benchmark_group("parse_err_unrendered");
    for &size in &sizes {
        let bytes = failing_blob(size);
        err.throughput(Throughput::Bytes(bytes.len() as u64));
        err.bench_with_input(BenchmarkId::from_parameter(size), &bytes, |b, bytes| {
            b.iter(|| {
                let r = parse(black_box(bytes));
                black_box(r.is_err());
            });
        });
    }
    err.finish();

    // Failure path, error rendered to a message — the realistic "log why it
    // failed" case. This is where cost scales with the binary size.
    let mut err_fmt = c.benchmark_group("parse_err_rendered");
    for &size in &sizes {
        let bytes = failing_blob(size);
        err_fmt.throughput(Throughput::Bytes(bytes.len() as u64));
        err_fmt.bench_with_input(BenchmarkId::from_parameter(size), &bytes, |b, bytes| {
            b.iter(|| {
                let msg = match parse(black_box(bytes)) {
                    Ok(_) => unreachable!("blob is intentionally invalid"),
                    Err(e) => e.to_string(),
                };
                black_box(msg);
            });
        });
    }
    err_fmt.finish();
}

criterion_group!(benches, bench);
criterion_main!(benches);
