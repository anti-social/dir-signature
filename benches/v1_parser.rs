#![feature(test)]

use std::fs::File;
use std::io::BufReader;

extern crate test;
use test::Bencher;

extern crate dir_signature;
use dir_signature::v1::{Entry, Parser};

#[bench]
fn bench_parser_iterator_ubuntu(bencher: &mut Bencher) {
    let mut num_iters = 0;
    let mut num_dirs = 0;
    let mut num_files = 0;
    let mut num_links = 0;
    bencher.iter(|| {
        let idx_file = File::open("benches/ubuntu-xenial.v1.idx").unwrap();
        let reader = BufReader::new(idx_file);
        let mut signature_parser = Parser::new(reader).unwrap();
        for entry in signature_parser.iter() {
            match entry.unwrap() {
                Entry::Dir(_) => num_dirs += 1,
                Entry::File(..) => num_files += 1,
                Entry::Link(..) => num_links += 1,
            }
        }
        num_iters += 1;
    });
    println!("");
    println!("{} iterations", num_iters);
    println!("{} dirs processed", num_dirs);
    println!("{} files processed", num_files);
    println!("{} links processed", num_links);
}
