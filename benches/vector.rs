#![feature(test)]
extern crate test;
use test::Bencher;
extern crate tempdir;
use self::tempdir::TempDir;
use std::path::PathBuf;

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

#[bench]
fn persist_10_000_usize(b: &mut Bencher) {
    let tmp = TempDir::new("freezer_bench").unwrap();
    let path = PathBuf::from(&tmp.path());
    let mut vec = Vector::new(path);
    for i in 0..10_000 {
        vec.push(i).unwrap();
    }
    b.iter(|| vec.persist().unwrap());
}


#[bench]
fn restore_and_get_1_000_000_usize(b: &mut Bencher) {
    let tmp = TempDir::new("freezer_bench").unwrap();
    let path = PathBuf::from(&tmp.path());
    let mut vec = Vector::<usize, PathBuf>::new(path.clone());
    for i in 0..1_000_000 {
        vec.push(i).unwrap();
    }
    let hash = vec.persist().unwrap();

    b.iter(|| {
               let restored = Vector::<usize, PathBuf>::restore(&hash,
                                                                path.clone())
                       .unwrap();
               assert_eq!(*restored.get(487).unwrap().unwrap(), 487);
           })
}
