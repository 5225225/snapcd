use rand::prelude::*;
use rand_chacha::ChaChaRng;
use snapcd::file::put_data;
use snapcd::DataStore;
use std::io::{self, Read};
use std::time::Duration;

use criterion::{criterion_group, criterion_main, BatchSize, Criterion, Throughput};

fn inner_bench<DS: DataStore, T: Fn() -> DS>(
    ctor: &T,
    bench: &mut Criterion,
    size: u64,
    bench_name: &str,
) {
    let rng: Box<dyn RngCore> = Box::new(ChaChaRng::seed_from_u64(1));
    let mut sample_data = rng.take(size);
    let mut buf = Vec::new();
    io::copy(&mut sample_data, &mut buf).unwrap();

    let mut g = bench.benchmark_group("put-test");

    g.throughput(Throughput::Bytes(size));

    g.sample_size(10);
    g.measurement_time(Duration::from_secs(20));

    g.bench_function(bench_name, |b| {
        b.iter_batched(
            || {
                let mut ds = ctor();
                ds.begin_trans().unwrap();
                ds
            },
            |mut data| put_data(&mut data, &buf[..]).unwrap(),
            BatchSize::PerIteration,
        )
    });

    g.finish();
}

#[allow(non_snake_case)]
fn perf_test_32B_sqlite_memory(bench: &mut Criterion) {
    let ctor = || snapcd::ds::sqlite::SqliteDS::new(":memory:").unwrap();
    inner_bench(&ctor, bench, 32, "put-data-32B-sqlite");
}

#[allow(non_snake_case)]
fn perf_test_4MB_sqlite_memory(bench: &mut Criterion) {
    let ctor = || snapcd::ds::sqlite::SqliteDS::new(":memory:").unwrap();
    inner_bench(&ctor, bench, 1 << 22, "put-data-4MB-sqlite");
}

#[allow(non_snake_case)]
fn perf_test_32B_null(bench: &mut Criterion) {
    let ctor = || snapcd::ds::null::NullDS;
    inner_bench(&ctor, bench, 32, "put-data-32B-null");
}

#[allow(non_snake_case)]
fn perf_test_4MB_null(bench: &mut Criterion) {
    let ctor = || snapcd::ds::null::NullDS;
    inner_bench(&ctor, bench, 1 << 22, "put-data-4MB-null");
}

criterion_group!(
    sqlite,
    perf_test_32B_sqlite_memory,
    perf_test_4MB_sqlite_memory
);
criterion_group!(null, perf_test_32B_null, perf_test_4MB_null);

criterion_main!(sqlite, null);
