#![feature(test)]

use std::io::{BufReader, Cursor, SeekFrom};

extern crate test;
use test::Bencher;

extern crate dir_signature;
use dir_signature::HashType;
use dir_signature::v1::{Entry, Parser, ParseError};

const content: &[u8] = b"\
DIRSIGNATURE.v1 sha512/256 block_size=32768
/
  empty.txt f 0
  hello.txt f 6 8dd499a36d950b8732f85a3bffbc8d8bee4a0af391e8ee2bb0aa0c4553b6c0fc
/subdir
  .hidden f 58394 24f72d3a930b5f7933ddd91a5c7cb7ba09a093f936a04bf6486c8b1763c59819 9ce28248299290fe84340d7821adf01b3b6a579ef827e1e58bc3949de4b7e5d9
  link s ../hello.txt
";

// #[bench]
// fn bench_parser_next(bencher: &mut Bencher) {
//     let mut num_iters = 0;
//     let mut num_dirs = 0;
//     let mut num_files = 0;
//     let mut num_links = 0;
//     bencher.iter(|| {
//         let reader = BufReader::new(Cursor::new(&content[..]));
//         let mut signature_parser = Parser::new(reader).unwrap();
//         // for entry in signature_parser.next() {
//         //     match entry.unwrap() {
//         //         Entry::Dir(_) => num_dirs += 1,
//         //         Entry::File(..) => num_files += 1,
//         //         Entry::Link(..) => num_links += 1,
//         //     }
//         // }
//         num_iters += 1;
//     });
//     println!("");
//     println!("{} iterations", num_iters);
//     println!("{} dirs processed", num_dirs);
//     println!("{} files processed", num_files);
//     println!("{} links processed", num_links);
// }

#[bench]
fn bench_parser_advance(bencher: &mut Bencher) {
    let reader = BufReader::new(Cursor::new(&content[..]));
    let mut signature_parser = Parser::new(reader).unwrap();

    let mut num_iters = 0;
    bencher.iter(|| {
        signature_parser.reset();
        signature_parser.advance("/zzz");
        num_iters += 1;
    });
    println!("{} iterations", num_iters);
}
