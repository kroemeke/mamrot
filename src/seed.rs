use std::fs::{File, OpenOptions};
use std::io::{self, Read};
use std::os::unix::fs::FileExt;
use std::sync::atomic::{AtomicUsize, Ordering};

/// Handles logging of random seeds to a ring-buffer file.
#[derive(Debug)]
pub struct SeedLog {
    file: File,
    buffer_size: usize,
    index: AtomicUsize,
}

impl SeedLog {
    /// Opens or creates the seed log file and prepares it for ring-buffer writing.
    pub fn new(path: &str, buffer_size: usize) -> io::Result<Self> {
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(true)
            .open(path)?;

        // Ensure file is large enough (buffer_size * 8 bytes)
        let file_size = (buffer_size * 8) as u64;
        file.set_len(file_size)?;

        Ok(Self {
            file,
            buffer_size,
            index: AtomicUsize::new(0),
        })
    }

    /// Writes a seed to the next position in the ring buffer.
    /// This uses `pwrite` (via FileExt) for thread-safe offsets without seeking.
    pub fn log(&self, seed: u64) {
        let idx = self.index.fetch_add(1, Ordering::Relaxed) % self.buffer_size;
        let offset = (idx * 8) as u64;

        // We ignore write errors to keep the fuzzer running fast.
        // In a perfect world, we might log this to stderr, but under load
        // we prioritize the loop speed.
        let _ = self.file.write_at(&seed.to_le_bytes(), offset);
    }
}

/// Reads all valid (non-zero) seeds from a binary seed log file.
pub fn load_seeds(path: &str) -> io::Result<Vec<u64>> {
    let mut file = File::open(path)?;
    let mut buffer = Vec::new();
    file.read_to_end(&mut buffer)?;

    let mut seeds = Vec::new();

    for chunk in buffer.chunks_exact(8) {
        // Safe because chunks_exact ensures 8 bytes
        let bytes: [u8; 8] = chunk.try_into().unwrap();
        let seed = u64::from_le_bytes(bytes);

        // Filter out zero seeds (empty/uninitialized parts of the ring buffer)
        if seed != 0 {
            seeds.push(seed);
        }
    }

    if seeds.is_empty() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("Seed file '{}' contains no valid seeds", path),
        ));
    }

    Ok(seeds)
}
