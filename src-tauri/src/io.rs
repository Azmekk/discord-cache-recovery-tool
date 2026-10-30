//! Positional, read-only file access.

use std::fs::{File, OpenOptions};
use std::io;
use std::path::Path;

/// Opens a file strictly read-only. On Windows the file is shared for read, write and
/// delete so a running Discord client is never blocked by us.
pub fn open_ro(path: &Path) -> io::Result<File> {
    let mut opts = OpenOptions::new();
    opts.read(true);
    #[cfg(windows)]
    {
        use std::os::windows::fs::OpenOptionsExt;
        // FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE
        opts.share_mode(0x1 | 0x2 | 0x4);
    }
    opts.open(path)
}

#[cfg(windows)]
fn pread(file: &File, buf: &mut [u8], offset: u64) -> io::Result<usize> {
    use std::os::windows::fs::FileExt;
    file.seek_read(buf, offset)
}

#[cfg(unix)]
fn pread(file: &File, buf: &mut [u8], offset: u64) -> io::Result<usize> {
    use std::os::unix::fs::FileExt;
    file.read_at(buf, offset)
}

/// Reads into `buf` starting at `offset`, looping until the buffer is full or EOF.
/// Returns the number of bytes read (less than `buf.len()` only at EOF).
pub fn read_at(file: &File, offset: u64, buf: &mut [u8]) -> io::Result<usize> {
    let mut done = 0;
    while done < buf.len() {
        match pread(file, &mut buf[done..], offset + done as u64) {
            Ok(0) => break,
            Ok(n) => done += n,
            Err(e) if e.kind() == io::ErrorKind::Interrupted => {}
            // Windows reports reads past EOF as an error on some handles.
            Err(e) if e.raw_os_error() == Some(38) => break, // ERROR_HANDLE_EOF
            Err(e) => return Err(e),
        }
    }
    Ok(done)
}

/// Reads up to `len` bytes at `offset` into a new vector (shorter at EOF).
pub fn read_vec(file: &File, offset: u64, len: usize) -> io::Result<Vec<u8>> {
    let mut buf = vec![0u8; len];
    let n = read_at(file, offset, &mut buf)?;
    buf.truncate(n);
    Ok(buf)
}

#[inline]
pub fn u32le(b: &[u8], off: usize) -> u32 {
    u32::from_le_bytes(b[off..off + 4].try_into().unwrap())
}

#[inline]
pub fn i32le(b: &[u8], off: usize) -> i32 {
    i32::from_le_bytes(b[off..off + 4].try_into().unwrap())
}

#[inline]
pub fn u64le(b: &[u8], off: usize) -> u64 {
    u64::from_le_bytes(b[off..off + 8].try_into().unwrap())
}

#[inline]
pub fn i64le(b: &[u8], off: usize) -> i64 {
    i64::from_le_bytes(b[off..off + 8].try_into().unwrap())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn reads_positionally_and_stops_at_eof() {
        let mut f = tempfile::NamedTempFile::new().unwrap();
        f.write_all(b"0123456789").unwrap();
        let file = open_ro(f.path()).unwrap();
        assert_eq!(read_vec(&file, 3, 4).unwrap(), b"3456");
        assert_eq!(read_vec(&file, 8, 10).unwrap(), b"89");
        assert!(read_vec(&file, 20, 4).unwrap().is_empty());
    }
}
