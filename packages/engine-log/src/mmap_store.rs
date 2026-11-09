use memmap2::Mmap;
use std::fs::File;
use std::io;
use std::path::Path;

pub struct MmapStore {
    mmap: Mmap,
    len: usize,
}

impl MmapStore {
    pub fn open<P: AsRef<Path>>(path: P) -> io::Result<Self> {
        let file = File::open(path)?;
        let mmap = unsafe { Mmap::map(&file)? };
        let len = mmap.len();
        Ok(Self { mmap, len })
    }

    #[inline]
    pub fn as_slice(&self) -> &[u8] {
        &self.mmap[..self.len]
    }

    #[inline]
    pub fn slice_at(&self, offset: usize, len: usize) -> Option<&[u8]> {
        if offset + len <= self.len {
            Some(&self.mmap[offset..offset + len])
        } else {
            None
        }
    }
}
