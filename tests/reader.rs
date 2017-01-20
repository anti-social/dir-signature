use std::io::Cursor;
use std::path::Path;

extern crate dir_signature;
use dir_signature::v1::{Entry, Reader};

#[test]
fn test_reader() {
    let content = b"\
DIRSIGNATURE.v1 sha512/256 block_size=32768
/
  empty.txt f 0
  hello.txt f 6 8dd499a36d950b8732f85a3bffbc8d8bee4a0af391e8ee2bb0aa0c4553b6c0fc
/subdir
  .hidden f 58394 24f72d3a930b5f7933ddd91a5c7cb7ba09a093f936a04bf6486c8b1763c59819 9ce28248299290fe84340d7821adf01b3b6a579ef827e1e58bc3949de4b7e5d9
  link s ../hello.txt
";
    // let cursor = Cursor::new(content);
    // let signature_reader = Reader::new(cursor);
    let signature_reader = Reader::new(&content[..]);
    let header = signature_reader.header();
    assert_eq!(header.get_version().unwrap(), "v1");
    assert_eq!(header.get_hash_type().unwrap(), "sha512/256");
    assert_eq!(header.get_block_size().unwrap(), 32768);
    let entry = signature_reader.next().unwrap();
    match entry {
        Entry::Dir(dir) => {
            assert_eq!(dir, Path::new("/"));
        },
        _ => {
            panic!("Expected directory");
        }
    }
    let entry = signature_reader.next().unwrap();
    match entry {
        Entry::File(path, size, hashes) => {
            assert_eq!(path, Path::new("/empty.txt"));
            assert_eq!(size, 0);
            assert!(hashes.next().is_none());
        },
        _ => {
            panic!("Expected file")
        }
    }
    let entry = signature_reader.next().unwrap();
    match entry {
        Entry::File(path, size, hashes) => {
            assert_eq!(path, Path::new("/hello.txt"));
            assert_eq!(size, 6);
            assert_eq!(hashes.next().unwrap(), "8dd499a36d950b8732f85a3bffbc8d8bee4a0af391e8ee2bb0aa0c4553b6c0fc");
            assert!(hashes.next().is_none());
        },
        _ => {
            panic!("Expected file")
        }
    }
    assert_eq!(signature_reader.advance("/subdir/link").unwrap(), ());
    let entry = signature_reader.next().unwrap();
    match entry {
        Entry::Link(path, dest) => {
            assert_eq!(path, Path::new("/subdir/link"));
            assert_eq!(dest, Path::new("../hello.txt"));
        },
        _ => {
            panic!("Expected symlink")
        }
    }
}

#[test]
fn test_reader_invalid_header_signature() {
    let content = "DIRSIGNATUR.v1 sha512/256 block_size=32768";
    let cursor = Cursor::new(content);
    let signature_reader = Reader::new(cursor);
    let header = signature_reader.header();
}
