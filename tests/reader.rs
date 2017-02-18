use std::io::{BufReader, Cursor};
use std::path::Path;

extern crate dir_signature;
use dir_signature::HashType;
use dir_signature::v1::{Entry, Parser, ParseError};

#[test]
fn test_parser() {
    let content = b"\
DIRSIGNATURE.v1 sha512/256 block_size=32768
/
  empty.txt f 0
  hello.txt f 6 8dd499a36d950b8732f85a3bffbc8d8bee4a0af391e8ee2bb0aa0c4553b6c0fc
/subdir
  .hidden f 58394 24f72d3a930b5f7933ddd91a5c7cb7ba09a093f936a04bf6486c8b1763c59819 9ce28248299290fe84340d7821adf01b3b6a579ef827e1e58bc3949de4b7e5d9
  link s ../hello.txt
";
    let reader = BufReader::new(Cursor::new(&content[..]));
    let mut signature_parser = Parser::new(reader).unwrap();

    let header = signature_parser.get_header();
    assert_eq!(header.get_version(), "v1");
    assert_eq!(header.get_hash_type(), HashType::Sha512_256);
    assert_eq!(header.get_block_size(), 32768);

    let entry = signature_parser.next().unwrap().unwrap();
    match entry {
        Entry::Dir(dir) => {
            assert_eq!(dir, Path::new("/"));
        },
        _ => {
            panic!("Expected directory");
        }
    }

    let entry = signature_parser.next().unwrap().unwrap();
    match entry {
        Entry::File(path, size, mut hashes) => {
            assert_eq!(path, Path::new("/empty.txt"));
            assert_eq!(size, 0);
            assert!(hashes.iter().next().is_none());
        },
        _ => {
            panic!("Expected file")
        }
    }

    let entry = signature_parser.next().unwrap().unwrap();
    match entry {
        Entry::File(path, size, mut hashes) => {
            let mut hashes_iter = hashes.iter();
            assert_eq!(path, Path::new("/hello.txt"));
            assert_eq!(size, 6);
            assert_eq!(hashes_iter.next().unwrap(),
                "8dd499a36d950b8732f85a3bffbc8d8bee4a0af391e8ee2bb0aa0c4553b6c0fc");
            assert!(hashes_iter.next().is_none());
        },
        _ => {
            panic!("Expected file")
        }
    }

    let entry = signature_parser.advance("/subdir/link").unwrap().unwrap();
    match entry {
        Entry::Link(path, dest) => {
            assert_eq!(path, Path::new("/subdir/link"));
            assert_eq!(dest, Path::new("../hello.txt"));
        },
        _ => {
            panic!("Expected symlink")
        }
    }

    assert!(signature_parser.advance("/subdir/link").unwrap().is_none());
    assert!(signature_parser.next().is_none());
}

#[test]
fn test_parser_invalid_header_signature() {
    let content = "DIRSIGNATUR.v1 sha512/256 block_size=32768";
    let reader = BufReader::new(Cursor::new(&content[..]));
    match Parser::new(reader) {
        Err(ParseError::Parse(msg, row_num)) => {
            assert_eq!(msg,
                "Invalid signature: expected \"DIRSIGNATURE\" but was \"DIRSIGNATUR\"");
            assert_eq!(row_num, 1);
        },
        Err(_) => {
            panic!("Expected \"ParseError::Parse\" error");
        },
        Ok(_) => {
            panic!("Expected error");
        },
    }
}
