#![feature(test)]

extern crate test;

use rand::prelude::*;
use rand_chacha::ChaChaRng;
use snapcd::file::put_data;
use snapcd::NullB2DS;
use std::io::{self, Read};

fn inner_bench(bench: &mut test::Bencher, size: u64) {
    let mut data = NullB2DS::default();

    let rng: Box<dyn RngCore> = Box::new(ChaChaRng::seed_from_u64(1));
    let mut sample_data = rng.take(size);
    let mut buf = Vec::new();
    io::copy(&mut sample_data, &mut buf).unwrap();

    bench.bytes = size;

    bench.iter(|| {
        test::black_box(put_data(&mut data, &buf[..]).unwrap());
    });
}

#[bench]
#[allow(non_snake_case)]
fn perf_test_32B(bench: &mut test::Bencher) {
    inner_bench(bench, 32);
}

#[bench]
#[allow(non_snake_case)]
fn perf_test_64KB(bench: &mut test::Bencher) {
    inner_bench(bench, 1 << 16);
}

#[bench]
#[allow(non_snake_case)]
fn perf_test_1MB(bench: &mut test::Bencher) {
    inner_bench(bench, 1 << 20);
}

#[bench]
#[allow(non_snake_case)]
fn perf_test_16MB(bench: &mut test::Bencher) {
    inner_bench(bench, 1 << 24);
}
