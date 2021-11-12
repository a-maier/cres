use std::io::{Read, Result, Seek, SeekFrom, Write};
use std::path::Path;
use std::fs::{Metadata, Permissions};

use crate::traits::TryClone;

/// This is a newtype wrapper around [std::fs::File]
///
/// The only reason for this is that we cannot implement
/// [TryClone](crate::traits::TryClone) on [std::fs::File]
#[derive(Debug)]
pub struct File(pub std::fs::File);

// TODO?:
// std::net::{TcpStream, TcpListener, UdpSocket};
// std::os::unix::net::{UnixDatagram, UnixListener, UnixStream};

impl TryClone for File {
    type Error = std::io::Error;

    fn try_clone(&self) -> Result<Self> {
        std::fs::File::try_clone(&self.0).map(File)
    }
}

impl File {

    pub fn open<P: AsRef<Path>>(path: P) -> Result<File> {
        std::fs::File::open(path).map(Self)
    }
    pub fn create<P: AsRef<Path>>(path: P) -> Result<File> {
        std::fs::File::create(path).map(Self)
    }
    pub fn sync_all(&self) -> Result<()> {
        self.0.sync_all()
    }
    pub fn sync_data(&self) -> Result<()> {
        self.0.sync_data()
    }
    pub fn set_len(&self, size: u64) -> Result<()> {
        self.0.set_len(size)
    }
    pub fn metadata(&self) -> Result<Metadata> {
        self.0.metadata()
    }
    pub fn set_permissions(&self, perm: Permissions) -> Result<()> {
        self.0.set_permissions(perm)
    }
}

impl Read for File {
    fn read(&mut self, buf: &mut [u8]) -> Result<usize> {
        self.0.read(buf)
    }
}

impl Read for &File {
    fn read(&mut self, buf: &mut [u8]) -> Result<usize> {
        (&self.0).read(buf)
    }
}

impl Seek for File {
    fn seek(&mut self, pos: SeekFrom) -> Result<u64> {
        self.0.seek(pos)
    }
}

impl Seek for &File {
    fn seek(&mut self, pos: SeekFrom) -> Result<u64> {
        (&self.0).seek(pos)
    }
}

impl Write for File {
    fn write(&mut self, buf: &[u8]) -> Result<usize> {
        self.0.write(buf)
    }

    fn flush(&mut self) -> Result<()> {
        self.0.flush()
    }
}

impl Write for &File {
    fn write(&mut self, buf: &[u8]) -> Result<usize> {
        (&self.0).write(buf)
    }

    fn flush(&mut self) -> Result<()> {
        (&self.0).flush()
    }
}
