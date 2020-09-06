use ck3save::Ck3Extractor;
use criterion::{criterion_group, criterion_main, Criterion, Throughput};

const HEADER_BIN: &'static [u8] = include_bytes!("../tests/fixtures/header.bin");
const HEADER_TXT: &'static [u8] = include_bytes!("../tests/fixtures/header.txt");

pub fn text_header_benchmark(c: &mut Criterion) {
    let mut group = c.benchmark_group("text-header");
    group.throughput(Throughput::Bytes(HEADER_TXT.len() as u64));
    group.bench_function("owned", |b| {
        b.iter(|| {
            Ck3Extractor::builder()
                .extract_header_owned(HEADER_TXT)
                .unwrap()
        });
    });
    group.bench_function("borrowed", |b| {
        b.iter(|| {
            Ck3Extractor::builder()
                .extract_header_borrowed(HEADER_TXT)
                .unwrap()
        });
    });
    group.finish();
}

pub fn binary_header_benchmark(c: &mut Criterion) {
    let mut group = c.benchmark_group("binary-header");
    group.throughput(Throughput::Bytes(HEADER_BIN.len() as u64));
    group.bench_function("owned", |b| {
        b.iter(|| {
            Ck3Extractor::builder()
                .extract_header_owned(HEADER_BIN)
                .unwrap()
        });
    });
    group.bench_function("borrowed", |b| {
        b.iter(|| {
            Ck3Extractor::builder()
                .extract_header_borrowed(HEADER_BIN)
                .unwrap()
        });
    });
    group.finish();
}

criterion_group!(benches, text_header_benchmark, binary_header_benchmark);
criterion_main!(benches);
