use std::borrow::Cow;
use std::error::Error;
use std::ffi::OsStr;
use std::fmt;
use std::io;
use std::io::BufRead;
use std::path::{Path, PathBuf};

use quick_error::ResultExt;

use ::HashType;
use super::writer::MAGIC;


macro_rules! itry {
    ($x: expr) => {
        match $x {
            Err(e) => return Some(Err(From::from(e))),
            Ok(v) => v,
        }
    }
}

#[derive(Debug)]
pub struct ParseRowError(String);

impl fmt::Display for ParseRowError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "Parse row error: {}", self.0)
    }
}

impl Error for ParseRowError {
    fn description(&self) -> &str {
        return &self.0;
    }
}

quick_error! {
    #[derive(Debug)]
    pub enum ParseError {
        Read(err: io::Error) {
            cause(err)
            description("error reading buffer")
            display("Error reading buffer: {}", err)
            from()
        }
        Parse(msg: String, row_num: usize) {
            description("parse error")
            display("Parse error at line {}: {}", row_num, msg)
            context(row_num: usize, err: ParseRowError)
                -> (err.0, row_num)
        }
    }
}

/// Entry hashes iterator
pub struct Hashes(String);

impl Iterator for Hashes {
    type Item = Cow<'static, str>;

    fn next(&mut self) -> Option<Self::Item> {
        None
    }
}

/// Represents an entry from dir signature file
pub enum Entry {
    /// Direcory
    Dir(PathBuf),
    /// File
    File(PathBuf, usize, Hashes),
    /// Link
    Link(PathBuf, PathBuf),
}

impl Entry {
    pub fn parse(row: &str) -> Result<Entry, ParseRowError> {
        // if row.starts_with('/') {
        //     return Ok(Entry::Dir(PathBuf::from(row)));
        // } else if row.starts_with("  ") {
        //     let row = &row[2..];
        //     let (path, row) = parse_str(row)?;
        // }
        Err(ParseRowError(format!("Expected \"/\" or \"  \" (two whitespaces)")))
    }
}

/// Represents header of the dir signature file
#[derive(Clone)]
pub struct Header {
    version: String,
    hash_type: HashType,
    block_size: u32,
}

impl Header {
    pub fn parse(line: &str) -> Result<Header, ParseRowError> {
        let mut parts = line.split(' ');
        let version = if let Some(signature) = parts.next() {
            let mut sig_parts = signature.splitn(2, '.');
            if let Some(magic) = sig_parts.next() {
                if magic != MAGIC {
                    return Err(ParseRowError(
                        format!("Invalid signature: expected {:?} but was {:?}",
                            MAGIC, magic)));
                }
            }
            if let Some(version) = sig_parts.next() {
                version
            } else {
                return Err(ParseRowError("Missing version".to_string()));
            }
        } else {
            return Err(ParseRowError("Invalid header".to_string()));
        };
        // TODO: parse other fields
        Ok(Header {
            version: version.to_string(),
            hash_type: HashType::Sha512_256,
            block_size: 32768,
        })
    }

    pub fn get_version(&self) -> &str {
        &self.version
    }

    pub fn get_hash_type(&self) -> HashType {
        self.hash_type
    }

    pub fn get_block_size(&self) -> u32 {
        self.block_size
    }
}

/// v1 format reader
pub struct Parser<R: BufRead> {
    header: Header,
    reader: R,
    current_row_num: usize,
}

impl<R: BufRead> Parser<R> {
    pub fn new(mut reader: R) -> Result<Parser<R>, ParseError> {
        let mut header_line = String::new();
        reader.read_line(&mut header_line)?;
        Ok(Parser {
            header: Header::parse(&header_line).context(1)?,
            reader: reader,
            current_row_num: 1
        })
    }

    pub fn get_header(&self) -> Header {
        self.header.clone()
    }

    pub fn advance<P: AsRef<Path>>(&mut self, path: P) -> Result<(), ParseError> {
        Ok(())
    }
}

impl<R: BufRead> Iterator for Parser<R> {
    type Item = Result<Entry, ParseError>;

    fn next(&mut self) -> Option<Self::Item> {
        let mut line = String::new();
        itry!(self.reader.read_line(&mut line));
        self.current_row_num += 1;
        let entry = itry!(Entry::parse(&line).context(self.current_row_num));
        Some(Ok(entry))
    }
}

// fn parse_str<'a>(row: &'a str)
//                  -> Result<(Cow<'a, str>, &'a str), ParseRowError>
// {
//     let (field, tail) = try!(parse_field(data, b" "));
//     Ok((unescape_hex(OsStr::from_bytes(field)), tail))
// }

// fn unescape_hex(s: &str) -> Cow<str> {
// }

fn is_hex_encoding(s: &[u8]) -> bool {
    s.len() >= 4 && s[0] == b'\\' && s[1] == b'x'
        && is_hex(s[2]) & is_hex(s[3])
}

fn is_hex(c: u8) -> bool {
    c >= b'0' && c <= b'9'
        || c >= b'A' && c <= b'F'
        || c >= b'a' && c <= b'f'
}

#[cfg(test)]
mod test {
    use super::{is_hex_encoding, is_hex};

    #[test]
    fn test_is_hex() {
        assert!(is_hex(b'0'));
        assert!(is_hex(b'9'));
        assert!(is_hex(b'A'));
        assert!(is_hex(b'F'));
        assert!(is_hex(b'a'));
        assert!(is_hex(b'f'));
        assert!(!is_hex(b'G'));
        assert!(!is_hex(b'x'));
        assert!(!is_hex(b'\\'));
        assert!(!is_hex(b' '));
    }
}
