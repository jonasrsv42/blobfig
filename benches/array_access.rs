//! Reading a big f32 array out of a parsed blob, the way a model loader does:
//! a zero-copy ndarray view if the data is aligned for f32, otherwise a copy.
//!
//! The blob sits in a 64-byte aligned buffer, as an mmap'd file would, so only
//! where the data falls inside the blob decides whether the view works.
//!
//! Run with: `cargo bench --features ndarray --bench array_access`

use blobfig::{Array, DType, NodeView, Value, parse, writer};
use criterion::{BenchmarkId, Criterion, Throughput, criterion_group, criterion_main};
use std::hint::black_box;

/// A buffer whose start is 64-byte aligned
#[repr(C, align(64))]
#[derive(Clone, Copy)]
struct Block([u8; 64]);

struct Aligned {
    blocks: Vec<Block>,
    len: usize,
}

impl Aligned {
    fn new(bytes: &[u8]) -> Self {
        let mut blocks = vec![Block([0; 64]); bytes.len().div_ceil(64)];
        for (block, chunk) in blocks.iter_mut().zip(bytes.chunks(64)) {
            block.0[..chunk.len()].copy_from_slice(chunk);
        }
        Aligned {
            blocks,
            len: bytes.len(),
        }
    }

    fn bytes(&self) -> &[u8] {
        // SAFETY: the blocks are `len.div_ceil(64) * 64` initialized bytes
        unsafe { std::slice::from_raw_parts(self.blocks.as_ptr().cast(), self.len) }
    }
}

/// A blob with one f32 array of `n` elements under the key "weights"
fn weights_blob(n: usize) -> Aligned {
    let data: Vec<u8> = (0..n).flat_map(|i| (i as f32).to_le_bytes()).collect();
    let array = Array::new(DType::F32, vec![n as u64], data);
    let value = Value::object(vec![("weights".into(), Value::Array(array))]);
    Aligned::new(&writer::to_bytes(value).unwrap())
}

fn bench(c: &mut Criterion) {
    let sizes = [1 << 20, 16 << 20]; // 1M and 16M f32 (4 MiB and 64 MiB)

    let mut group = c.benchmark_group("f32 weights");
    for &n in &sizes {
        let blob = weights_blob(n);
        let parsed = parse(blob.bytes()).unwrap();
        let weights = parsed.array("weights").unwrap();
        let aligned = weights.try_as_ndarray::<f32>().is_ok();
        println!("{n} f32: data at offset {}, zero-copy view works: {aligned}",
            weights.data.as_ptr() as usize - blob.bytes().as_ptr() as usize);

        group.throughput(Throughput::Bytes((n * 4) as u64));

        // What a loader does: view if it can, copy if it must, then read it
        group.bench_with_input(BenchmarkId::new("view or copy, then sum", n), &n, |b, _| {
            b.iter(|| {
                let parsed = parse(black_box(blob.bytes())).unwrap();
                let weights = parsed.array("weights").unwrap();
                let sum: f32 = match weights.try_as_ndarray::<f32>() {
                    Ok(view) => view.sum(),
                    Err(_) => weights.to_ndarray::<f32>().unwrap().sum(),
                };
                black_box(sum)
            })
        });

        // The copy on its own, for comparison
        group.bench_with_input(BenchmarkId::new("copy (to_ndarray)", n), &n, |b, _| {
            b.iter(|| black_box(weights.to_ndarray::<f32>().unwrap()))
        });

        // Parsing alone: zero-copy, so it should not depend on n
        group.bench_with_input(BenchmarkId::new("parse only", n), &n, |b, _| {
            b.iter(|| black_box(parse(black_box(blob.bytes())).unwrap()))
        });
    }
    group.finish();
}

criterion_group!(benches, bench);
criterion_main!(benches);
