use std;
use std::borrow::Cow;
use std::cmp::Ordering;
use std::convert::From;
use std::error::Error;
use std::ffi::{OsStr, OsString};
use std::os::unix::ffi::{OsStrExt, OsStringExt};
use std::fmt;
use std::io;
use std::io::{BufRead, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::slice::Iter;
use std::str::{FromStr, Utf8Error};

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

impl From<Utf8Error> for ParseRowError {
    fn from(err: Utf8Error) -> ParseRowError {
        ParseRowError(format!("expected valid utf8 string: {}", err))
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

/// Represents header of the dir signature file
#[derive(Clone)]
pub struct Header {
    version: String,
    hash_type: HashType,
    block_size: u64,
}

impl Header {
    pub fn parse(row: &[u8]) -> Result<Header, ParseRowError> {
        let line = std::str::from_utf8(row)?.trim_right_matches('\n');
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
        let hash_type = if let Some(hash_type_str) = parts.next() {
            HashType::from_str(hash_type_str)
                .map_err(|e| ParseRowError(format!("{}", e)))?
        } else {
            return Err(ParseRowError(
                "Invalid header: missing hash type".to_string()));
        };
        let block_size = if let Some(block_size_attr) = parts.next() {
            let mut block_size_kv = block_size_attr.splitn(2, '=');
            match block_size_kv.next() {
                None => return Err(ParseRowError(
                    format!("Invalid header: missing block_size"))),
                Some(k) if k != "block_size" => return Err(ParseRowError(
                    format!("Invalid header: expected block_size attribute"))),
                Some(_) => {
                    let v = block_size_kv.next().unwrap();
                    // println!("block_size: {:?}", v);
                    u64::from_str_radix(v, 10)
                        .map_err(|e| ParseRowError(format!("Invalid header: {}", e)))?
                },
            }
        } else {
            return Err(ParseRowError(
                format!("Invalid header: missing block size attribute")));
        };
        // TODO: parse other fields
        Ok(Header {
            version: version.to_string(),
            hash_type: hash_type,
            block_size: block_size,
        })
    }

    pub fn get_version(&self) -> &str {
        &self.version
    }

    pub fn get_hash_type(&self) -> HashType {
        self.hash_type
    }

    pub fn get_block_size(&self) -> u64 {
        self.block_size
    }
}

/// Entry hashes iterator
#[derive(Debug)]
pub struct Hashes(Vec<String>);

impl Hashes {
    pub fn parse(row: &[u8]) -> Result<Hashes, ParseRowError> {
        let hashes_str = std::str::from_utf8(row)?.to_string();
        if hashes_str.is_empty() {
            Ok(Hashes(vec!()))
        } else {
            Ok(Hashes(hashes_str.split(' ')
                .map(|h| h.to_string())
                .collect::<Vec<_>>()))

        }
    }

    pub fn iter(&self) -> Iter<String> {
        self.0.iter()
    }
}

// struct HashesIterator {
//     hashes: String,
//     cur_pos: 0,
// }

// impl Iterator for HashesIterator {
//     type Item = Cow<'static, str>;

//     fn next(&mut self) -> Option<Self::Item> {
//         self.cur_pos
//     }
// }

/// Represents an entry from dir signature file
#[derive(Debug)]
pub enum Entry {
    /// Direcory
    Dir(PathBuf),
    /// File
    File(PathBuf, u64, Hashes),
    // File(PathBuf, bool, usize, Hashes),
    /// Link
    Link(PathBuf, PathBuf),
}

impl Entry {
    pub fn parse(row: &[u8], cur_dir: &Path) -> Result<Entry, ParseRowError> {
        let row = if row.ends_with(b"\n") {
            &row[..row.len()-1]
        } else {
            row
        };
        // println!("row: {}", String::from_utf8_lossy(row));
        let entry = if row.starts_with(b"/") {
            let (path, row) = parse_path_buf(row);
            Entry::Dir(path)
        } else if row.starts_with(b"  ") {
            let row = &row[2..];
            let (path, row) = parse_path_buf(row); // TODO: optimize
            let path = cur_dir.join(&path);
            let (file_type, row) = parse_os_str(row);
            if file_type == "f" || file_type == "x" {
                let (size, row) = parse_u64(row)?;
                let hashes = Hashes::parse(row)?;
                Entry::File(path, size, hashes)
            } else if file_type == "s" {
                let (dest, row) = parse_path_buf(row);
                Entry::Link(path, dest)
            } else {
                return Err(ParseRowError(
                    format!("Unknown file type: {:?}",
                        String::from_utf8_lossy(file_type.as_bytes()))))
            }
        } else {
            return Err(ParseRowError(
                format!("Expected \"/\" or \"  \" (two whitespaces)")));
        };
        Ok(entry)
    }
}

/// v1 format reader
pub struct Parser<R: BufRead + Seek> {
    header: Header,
    reader: R,
    current_dir: PathBuf,
    current_row_num: usize,
}

impl<R: BufRead + Seek> Parser<R> {
    pub fn new(mut reader: R) -> Result<Parser<R>, ParseError> {
        let mut header_line = vec!();
        reader.read_until(b'\n', &mut header_line)?;
        Ok(Parser {
            header: Header::parse(&header_line).context(1)?,
            reader: reader,
            current_dir: PathBuf::new(),
            current_row_num: 1,
        })
    }

    pub fn reset(&mut self) -> Result<(), io::Error> {
        self.reader.seek(SeekFrom::Start(0))?;
        self.current_dir = PathBuf::new();
        self.current_row_num = 1;
        let _header_line = self.next_line();
        Ok(())
    }

    pub fn get_header(&self) -> Header {
        self.header.clone()
    }

    pub fn advance<P: AsRef<Path>>(&mut self, path: P)
        -> Result<Option<Entry>, ParseError>
    {
        // let mut line = self.next_line()?;
        let mut path = path.as_ref();
        let mut skip_files = !path.starts_with(&self.current_dir);
        loop {
            let line = if let Some(line) = self.next_line()? {
                line
            } else {
                return Ok(None);
            };
            // println!("advance: {:?}", String::from_utf8_lossy(&line));
            self.current_row_num += 1;
            if line.starts_with(b"/") {
                let (dir_path, _) = parse_path(&line);
                // let (dir_path, _) = parse_os_str(&line);
                // let dir_path = OsStr::from_bytes(&line);
                match dir_path.partial_cmp(path) {
                    Some(Ordering::Less) => {
                        if path.starts_with(&dir_path) {
                            self.current_dir = dir_path.to_path_buf();
                            // path = path.strip_prefix(dir_path).unwrap();
                        } else {
                            skip_files = true;
                        }
                    },
                    Some(Ordering::Equal) => {
                        return Ok(Some(Entry::Dir(dir_path.to_path_buf())));
                    },
                    Some(Ordering::Greater) => {
                        return Ok(None);
                    },
                    None => unreachable!(),
                }
                continue;
            }
            if skip_files {
                continue;
            }
            if line.starts_with(b"  ") {
                let row = &line[2..];
                let (file_path, _) = parse_path(row);
                // println!("file: {:?}", file_path);
                // println!("current dir: {:?}", &self.current_dir);
                // println!("current file: {:?}", file_path.join(&self.current_dir));
                match self.current_dir.join(file_path).partial_cmp(path) {
                    Some(Ordering::Less) => {},
                    Some(Ordering::Equal) => {
                        return Ok(Some(Entry::parse(&line, &self.current_dir)
                            .context(self.current_row_num)?));
                    },
                    Some(Ordering::Greater) => {
                        return Ok(None);
                    },
                    None => unreachable!(),
                }
                continue;
            }
            return Err(ParseError::Parse(
                format!("Expected \"/\" or \"  \" (two whitespaces)"),
                self.current_row_num));
        }
        // println!("{:?}", dir_path);
    }

    fn next_line(&mut self) -> Result<Option<Vec<u8>>, ParseError> {
        let mut line = vec!();
        while line.is_empty() {
            if self.reader.read_until(b'\n', &mut line)? == 0 {
                return Ok(None);
            }
            if line.ends_with(b"\n") {
                line.pop();
            }
        }
        Ok(Some(line))
    }
}

impl<R: BufRead + Seek> Iterator for Parser<R> {
    type Item = Result<Entry, ParseError>;

    fn next(&mut self) -> Option<Self::Item> {
        let mut line = if let Some(line) = itry!(self.next_line()) {
            line
        } else {
            return None;
        };
        self.current_row_num += 1;
        let entry = itry!(Entry::parse(&line, &self.current_dir)
            .context(self.current_row_num));
        if let Entry::Dir(ref dir_path) = entry {
            self.current_dir = dir_path.clone();
        }
        Some(Ok(entry))
    }
}

// fn parse_str<'a>(row: &'a str)
//                  -> Result<(Cow<'a, str>, &'a str), ParseRowError>
// {
//     let (field, tail) = try!(parse_field(data, b" "));
//     Ok((unescape_hex(OsStr::from_bytes(field)), tail))
// }

fn parse_path<'a>(data: &'a [u8]) -> (&Path, &'a [u8]) {
    let (p, tail) = parse_os_str(data);
    (Path::new(p), tail)
 }

fn parse_path_buf<'a>(data: &'a [u8]) -> (PathBuf, &'a [u8]) {
    let (p, tail) = parse_os_str(data);
    (PathBuf::from(&p), tail)
}

fn parse_os_str<'a>(data: &'a [u8]) -> (&OsStr, &'a [u8]) {
    let (field, tail) = parse_field(data);
    (OsStr::from_bytes(field), tail)
}

fn parse_u64<'a>(data: &'a [u8]) -> Result<(u64, &'a [u8]), ParseRowError> {
    let (field, tail) = parse_field(data);
    let v = try!(std::str::from_utf8(field).map_err(|e| {
        ParseRowError(format!("Cannot parse integer {:?}: {}",
            String::from_utf8_lossy(field).into_owned(), e))}));

    let v = try!(u64::from_str_radix(v, 10).map_err(|e| {
        ParseRowError(format!("Cannot parse integer {:?}: {}",
            String::from_utf8_lossy(field).into_owned(), e))}));
    Ok((v, tail))
}

fn parse_field<'a>(data: &'a [u8]) -> (&'a [u8], &'a [u8]) {
    // println!("data: {:?}", std::str::from_utf8(data).unwrap());
    let mut parts = data.splitn(2, |c| *c == b' ');
    let field = parts.next().unwrap();
    let tail = parts.next().unwrap_or(&data[0..0]);
    (field, tail)
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
