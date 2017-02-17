use std::borrow::Cow;
use std::error::Error;
use std::ffi::{OsStr, OsString};
use std::os::unix::ffi::{OsStrExt, OsStringExt};
use std::fmt;
use std::io;
use std::io::BufRead;
use std::path::{Path, PathBuf};
use std::str::from_utf8;

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
    pub fn parse(row: &[u8]) -> Result<Entry, ParseRowError> {
        if row.starts_with(b"/") {
            return Ok(Entry::Dir(parse_path_buf(row)));
        }
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
    pub fn parse(line: &[u8]) -> Result<Header, ParseRowError> {
        let mut parts = line.split(|c| *c == b' ');
        let version = if let Some(signature) = parts.next() {
            let mut sig_parts = signature.splitn(2, |c| *c == b'.');
            if let Some(magic) = sig_parts.next() {
                if magic != MAGIC.as_bytes() {
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
            version: from_utf8(version).unwrap().to_string(),
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
        let mut header_line = vec!();
        reader.read_until(b'\n', &mut header_line)?;
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
        let mut line = vec!();
        itry!(self.reader.read_until(b'\n', &mut line));
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

fn parse_path_buf(data: &[u8]) -> PathBuf {
    let s = parse_os_str(data);
    PathBuf::from(s)
}

fn parse_os_str<'a>(data: &'a [u8]) -> (Cow<'a, OsStr>, &'a [u8]) {
    let (field, tail) = parse_field(data);
    (OsStr::from_bytes(field), tail)
}

fn parse_field<'a>(data: &'a [u8]) -> (Cow<'a, OsStr>, &'a [u8]) {
    data.split(|c| c == b' ').next().unwrap()
}

fn split_by<'a, 'b>(v: &'a [u8], needle: &'b [u8]) -> (&'a [u8], &'a [u8]) {
    if needle.len() > v.len() {
        return (&v[0..], &v[0..0]);
    }
    let mut i = 0;
    while i <= v.len() - needle.len() {
        let (head, tail) = v.split_at(i);
        if tail.starts_with(needle) {
            return (head, &tail[needle.len()..]);
        }
        i += 1;
    }
    return (&v[0..], &v[0..0]);
}

fn unescape_hex(s: &OsStr) -> Cow<OsStr> {
    // return Cow::Borrowed(s);
    let (mut i, has_escapes) = {
        let bytes = s.as_bytes();
        let mut i = 0;
        while i < bytes.len() {
            if is_hex_encoding(&bytes[i..]) {
                break;
            }
            i += 1;
        }
        (i, i < bytes.len())
    };
    if !has_escapes {
        return Cow::Borrowed(s);
    }

    let mut v: Vec<u8> = vec!();
    let bytes = s.as_bytes();
    v.extend_from_slice(&bytes[..i]);
    while i < bytes.len() {
        if is_hex_encoding(&bytes[i..]) {
            let c = parse_hex(&bytes[i + 2..]);
            v.push(c);
            i += 4;
        } else {
            v.push(bytes[i]);
            i += 1;
        }
    }
    Cow::Owned(OsString::from_vec(v))
}

fn parse_hex(v: &[u8]) -> u8 {
    (hex_to_digit(v[0]) << 4) | hex_to_digit(v[1])
}

fn hex_to_digit(v: u8) -> u8 {
    if v >= b'0' && v <= b'9' {
        return v & 0x0f;
    }
    return (v & 0x0f) + 9;
}

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
    use std::borrow::Cow;
    use std::ffi::OsStr;

    use super::{parse_hex, hex_to_digit, is_hex, is_hex_encoding, unescape_hex};

    #[test]
    fn test_parse_hex() {
        assert_eq!(parse_hex(b"00"), 0);
        assert_eq!(parse_hex(b"01"), 1);
        assert_eq!(parse_hex(b"0A"), 10);
        assert_eq!(parse_hex(b"0e"), 14);
        assert_eq!(parse_hex(b"1f"), 31);
        assert_eq!(parse_hex(b"7f"), 127);
        assert_eq!(parse_hex(b"fF"), 255);
        assert_eq!(parse_hex(b"00test"), 0);
    }

    #[test]
    fn test_hex_to_digit() {
        assert_eq!(hex_to_digit(b'0'), 0);
        assert_eq!(hex_to_digit(b'1'), 1);
        assert_eq!(hex_to_digit(b'9'), 9);
        assert_eq!(hex_to_digit(b'a'), 10);
        assert_eq!(hex_to_digit(b'A'), 10);
        assert_eq!(hex_to_digit(b'f'), 15);
        assert_eq!(hex_to_digit(b'F'), 15);
    }

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

    #[test]
    fn test_is_hex_encoding() {
        assert!(is_hex_encoding(br"\x00"));
        assert!(is_hex_encoding(br"\x00test"));
        assert!(is_hex_encoding(br"\x9f"));
        assert!(is_hex_encoding(br"\xfF"));
        assert!(!is_hex_encoding(br"\x"));
        assert!(!is_hex_encoding(br"\x0"));
        assert!(!is_hex_encoding(br"x001"));
        assert!(!is_hex_encoding(br"\00"));
        assert!(!is_hex_encoding(br"\xfg"));
        assert!(!is_hex_encoding(br"\xz1"));
    }

    #[test]
    fn test_unescape_hex() {
        let res = unescape_hex(OsStr::new("test"));
        assert_eq!(res, OsStr::new("test"));
        assert!(matches!(res, Cow::Borrowed(_)));
        let res = unescape_hex(OsStr::new("\\x0test"));
        assert_eq!(res, OsStr::new("\\x0test"));
        assert!(matches!(res, Cow::Borrowed(_)));
        let res = unescape_hex(OsStr::new("\\x00"));
        assert_eq!(res, OsStr::new("\x00"));
        assert!(matches!(res, Cow::Owned(_)));
        let res = unescape_hex(OsStr::new("test\\x20123"));
        assert_eq!(res, OsStr::new("test 123"));
        assert!(matches!(res, Cow::Owned(_)));
    }
}
