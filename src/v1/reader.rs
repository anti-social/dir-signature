use std::borrow::Cow;
use std::ffi::OsStr;

/// Entry hashes iterator
pub struct Hashes<'a>(Cow<'a, OsStr>);

/// Represents an entry from dir signature file
pub enum Entry<'a> {
    /// Direcory
    Dir(Cow<'a, OsStr>),
    /// File
    File(Cow<'a, OsStr>, usize, Hashes<'a>),
    /// Link
    Link(Cow<'a, OsStr>, Cow<'a, OsStr>),
}

/// Represents header of the dir signature file
pub struct Header<'a> {
    content: &'a [u8],
}

impl<'a> Header<'a> {
    pub fn get_version(self) -> Cow<'a, OsStr> {
    }
}

impl<'a> Iterator for Header<'a> {
    type Item = Cow<'a, OsStr>;

    fn next(&mut self) -> Option<Cow<'a, OsStr>> {
        None
    }
}

/// v1 format reader
pub struct Reader<'a> {
    header: Header<'a>,
    data: &'a [u8],
}

impl<'a> Reader<'a> {
    pub fn new(content: &'a [u8]) -> Reader<'a> {
        let mut header_and_data = content.splitn(2, |c| *c == b'\n');
        let header = header_and_data.next().unwrap();
        let data = header_and_data.next().unwrap();
        Reader {
            header: Header {
                content: header,
            },
            data: data,
        }
    }

    pub fn header(self) -> Header<'a> {
        self.header
    }
}

impl<'a> Iterator for Reader<'a> {
    type Item = Entry<'a>;

    fn next(&mut self) -> Option<Entry<'a>> {
        None
    }
}
