use std::fs::{File, OpenOptions};
use std::io::{self, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};

#[derive(Clone, Copy, Debug)]
pub struct SegmentConfig {
    pub segment_size: usize,
}

impl Default for SegmentConfig {
    fn default() -> Self {
        Self {
            segment_size: 64 * 1024 * 1024, // 64 MB default
        }
    }
}

pub struct SegmentLog {
    dir: PathBuf,
    config: SegmentConfig,
    active_segment_id: u64,
    current_file: File,
    write_offset: usize,
}

impl SegmentLog {
    pub fn open<P: AsRef<Path>>(dir: P, config: SegmentConfig) -> io::Result<Self> {
        let dir = dir.as_ref().to_path_buf();
        std::fs::create_dir_all(&dir)?;

        let segment_id = 0;
        let file_path = Self::segment_path(&dir, segment_id);
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(&file_path)?;

        let current_len = file.metadata()?.len() as usize;
        let write_offset = if current_len == 0 {
            file.set_len(config.segment_size as u64)?;
            0
        } else {
            current_len.min(config.segment_size)
        };

        Ok(Self {
            dir,
            config,
            active_segment_id: segment_id,
            current_file: file,
            write_offset,
        })
    }

    pub fn segment_path(dir: &Path, id: u64) -> PathBuf {
        dir.join(format!("{:020}.log", id))
    }

    #[inline]
    pub fn active_segment_id(&self) -> u64 {
        self.active_segment_id
    }

    #[inline]
    pub fn write_offset(&self) -> usize {
        self.write_offset
    }

    pub fn append(&mut self, data: &[u8]) -> io::Result<(u64, usize)> {
        let len = data.len();
        if self.write_offset + len > self.config.segment_size {
            self.rotate()?;
        }

        self.current_file.seek(SeekFrom::Start(self.write_offset as u64))?;
        self.current_file.write_all(data)?;
        let offset = self.write_offset;
        self.write_offset += len;
        Ok((self.active_segment_id, offset))
    }

    pub fn flush(&mut self) -> io::Result<()> {
        self.current_file.sync_data()
    }

    fn rotate(&mut self) -> io::Result<()> {
        self.flush()?;
        self.active_segment_id += 1;
        let file_path = Self::segment_path(&self.dir, self.active_segment_id);
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(&file_path)?;
        file.set_len(self.config.segment_size as u64)?;
        self.current_file = file;
        self.write_offset = 0;
        Ok(())
    }
}
