#![feature(test)]

extern crate test;

use rand::prelude::*;
use rand_chacha::ChaChaRng;
use std::io::{self, Read};
use snapcd::{NullB2DS, put_data};

fn inner_bench(bench: &mut test::Bencher, size: u64) {
    let mut data = NullB2DS::default();

    let rng: Box<dyn RngCore> = Box::new(ChaChaRng::seed_from_u64(1));
    let mut sample_data = rng.take(size);
    let mut buf = Vec::new();
    io::copy(&mut sample_data, &mut buf).unwrap();

    bench.bytes = size;

    bench.iter(|| {
        test::black_box(put_data(&buf[..], &mut data));
    });
}

#[bench]
#[allow(non_snake_case)]
fn perf_test_64KB(bench: &mut test::Bencher) {
    inner_bench(bench, 1<<16);
}

#[bench]
#[allow(non_snake_case)]
fn perf_test_1MB(bench: &mut test::Bencher) {
    inner_bench(bench, 1<<20);
}

#[bench]
#[allow(non_snake_case)]
fn perf_test_16MB(bench: &mut test::Bencher) {
    inner_bench(bench, 1<<24);
}
