#![feature(test)]
extern crate test;
use test::Bencher;

#[macro_use]
extern crate collection;

use collection::*;

collection!(Vector<T, BlakeWrap> {
    cardinality: Cardinality<usize>,
} where T: Sized);

#[inline]
fn add_n_usize(n: usize) {
    let mut a = Vector::new(());
    for i in 0..n {
        a.push(i).unwrap();
    }
}

#[bench]
fn add_1_usize(b: &mut Bencher) {
    b.iter(|| add_n_usize(1));
}

#[bench]
fn add_1_000_usize(b: &mut Bencher) {
    b.iter(|| add_n_usize(1_000));
}

#[bench]
fn add_1_000_000_usize(b: &mut Bencher) {
    b.iter(|| add_n_usize(1_000_000));
}
