use rand::seq::SliceRandom;
use rand::Rng;
use std::fs::File;
use std::io::{self, BufRead, BufReader};
use std::path::Path;

// Typical buffer sizes
static MAGIC_NUMBERS: [u64; 11] = [32, 64, 128, 256, 512, 1024, 2048, 4096, 8192, 32767, 65535];

// Format strings and header value separators
static MAGIC_STRINGS: [&str; 5] = ["%s%n%x", "%", ",", " ", "."];

#[derive(Debug, Default)]
pub struct Cube {
    int_size_1: u64,
    int_size_2: u64,
    int_size_3: u64,
    int_size_4: u64,
    int_size_5: u64,
    int_size_magic: u64,
    string_1: String,
    headers: Vec<String>,
}

impl Cube {
    pub fn new() -> Cube {
        Cube {
            ..Default::default()
        }
    }

    pub fn load_headers(mut self, filename: &str) -> io::Result<Self> {
        // Open file in ro mode
        let file = File::open(filename)?;

        // BufReader
        let reader = BufReader::new(file);

        // Load them up filtering unreadable lines
        self.headers = reader.lines().filter_map(Result::ok).collect();

        Ok(self)
    }

    pub fn rotate(&mut self) {
        let mut rng = rand::thread_rng();
        self.int_size_1 = rng.gen_range(1..=10);
        self.int_size_magic = (*MAGIC_NUMBERS.choose(&mut rng).expect("poop"));

        self.string_1.clear();
        for i in 1..self.int_size_magic + self.int_size_1 {
            self.string_1
                .push_str(MAGIC_STRINGS.choose(&mut rng).expect("poop"));
        }
    }
}
