use rand::seq::SliceRandom;
use rand::Rng;
use std::fs::File;
use std::io::{self, BufRead, BufReader};
use std::sync::Arc;

// Typical buffer sizes
static MAGIC_NUMBERS: [u64; 11] = [32, 64, 128, 256, 512, 1024, 2048, 4096, 8192, 32767, 65535];

// Format strings and header value separators
static MAGIC_STRINGS: [&str; 5] = ["%s%n%x", "%", ",", " ", "."];

// Separators for composite payloads
static SEPARATORS: [&str; 6] = [",", ";", "/", "", " ", "="];

// Integer edge cases (from Archer)
static MAGIC_PAYLOADS: [&str; 51] = [
    "0",
    "-1",
    "1",
    "-127",
    "-128",
    "-129",
    "127",
    "128",
    "129",
    "-254",
    "-255",
    "-256",
    "254",
    "255",
    "256",
    "-32766",
    "-32767",
    "-32768",
    "32766",
    "32767",
    "32768",
    "-65534",
    "-65535",
    "-65536",
    "65534",
    "65535",
    "65536",
    "-2147483646",
    "-2147483647",
    "-2147483648",
    "2147483646",
    "2147483647",
    "2147483648",
    "-4294967294",
    "-4294967295",
    "-4294967296",
    "4294967294",
    "4294967295",
    "4294967296",
    "-9223372036854775807",
    "-9223372036854775808",
    "-9223372036854775809",
    "9223372036854775807",
    "9223372036854775808",
    "9223372036854775809",
    "-18446744073709551614",
    "-18446744073709551615",
    "-18446744073709551616",
    "18446744073709551614",
    "18446744073709551615",
    "18446744073709551616",
];

// Nginx/OpenResty HTTP Methods
pub static NGINX_METHODS: [&str; 7] = ["GET", "POST", "PUT", "HEAD", "OPTIONS", "TRACE", "CONNECT"];

// Python's string.printable
const PRINTABLES: &[u8] = b"0123456789abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ!\"#$%&'()*+,-./:;<=>?@[\\]^_`{|}~ \t\n\r\x0b\x0c";

#[derive(Debug, Clone)]
pub struct Cube {
    pub int_size_1: u64,
    pub _int_size_2: u64,
    _int_size_3: u64,
    _int_size_4: u64,
    _int_size_5: u64,
    int_size_magic: u64,
    pub string_1: Vec<u8>,
    pub uri: Vec<u8>,
    pub headers: Arc<Vec<String>>,
    pub wordlist: Arc<Vec<String>>,
    pub methods: Arc<Vec<String>>,
    pub max_headers_count: u64,
}

impl Default for Cube {
    fn default() -> Self {
        Self {
            int_size_1: 0,
            _int_size_2: 0,
            _int_size_3: 0,
            _int_size_4: 0,
            _int_size_5: 0,
            int_size_magic: 0,
            string_1: Vec::with_capacity(4096),
            uri: Vec::with_capacity(128),
            headers: Arc::new(Vec::new()),
            wordlist: Arc::new(Vec::new()),
            methods: Arc::new(NGINX_METHODS.iter().map(|&s| s.to_string()).collect()),
            max_headers_count: 10,
        }
    }
}

impl Cube {
    pub fn new() -> Cube {
        Cube {
            ..Default::default()
        }
    }

    pub fn with_max_headers(mut self, count: u64) -> Self {
        self.max_headers_count = count;
        self
    }

    pub fn load_headers(mut self, filename: &str) -> io::Result<Self> {
        // Open file in ro mode
        let file = File::open(filename)?;

        // BufReader
        let reader = BufReader::new(file);

        // Load them up filtering unreadable lines
        let headers: Vec<String> = reader.lines().map_while(Result::ok).collect();

        if headers.is_empty() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("Header file '{}' is empty", filename),
            ));
        }
        self.headers = Arc::new(headers);

        Ok(self)
    }

    pub fn load_wordlist(mut self, filename: &str) -> io::Result<Self> {
        // Open file in ro mode
        let file = File::open(filename)?;

        // BufReader
        let reader = BufReader::new(file);

        // Load them up filtering unreadable lines
        let wordlist: Vec<String> = reader.lines().map_while(Result::ok).collect();

        if wordlist.is_empty() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("Wordlist file '{}' is empty", filename),
            ));
        }
        self.wordlist = Arc::new(wordlist);

        Ok(self)
    }

    pub fn rotate<R: Rng + ?Sized>(&mut self, rng: &mut R) {
        if self.max_headers_count > 0 {
            self.int_size_1 = rng.gen_range(1..=self.max_headers_count);
        } else {
            self.int_size_1 = 0;
        }
        self.int_size_magic = *MAGIC_NUMBERS
            .choose(rng)
            .expect("Internal Error: MAGIC_NUMBERS empty");

        self.string_1.clear();

        // Select a strategy
        let strategy = rng.gen_range(0..5);

        match strategy {
            0 => {
                // Strategy 1: Interleaved
                let iterations = (self.int_size_magic as usize / 10).max(1);
                for _ in 0..iterations {
                    self.string_1.extend_from_slice(
                        MAGIC_STRINGS
                            .choose(rng)
                            .expect("Internal Error: MAGIC_STRINGS empty")
                            .as_bytes(),
                    );
                    if !self.wordlist.is_empty() {
                        self.string_1.extend_from_slice(
                            self.wordlist
                                .choose(rng)
                                .expect("Internal Error: Wordlist empty during rotation")
                                .as_bytes(),
                        );
                    }
                }
            }
            1 => {
                // Strategy 2: Pure Repetition (Targeted Buffer Overflow)
                let magic = MAGIC_STRINGS.choose(rng).unwrap().as_bytes();

                // Add a small offset to the magic number to hit boundary conditions
                // or overflow slightly (e.g. 256, 257, etc.)
                let offset = rng.gen_range(0..=32);
                let target_len = (self.int_size_magic + offset) as usize;

                while self.string_1.len() < target_len {
                    self.string_1.extend_from_slice(magic);
                }
            }
            2 => {
                // Strategy 3: Character Flood
                let char_idx = rng.gen_range(0..PRINTABLES.len());
                let c = PRINTABLES[char_idx]; // direct u8

                let offset = rng.gen_range(0..=32);
                let target_len = (self.int_size_magic + offset) as usize;

                self.string_1.extend(std::iter::repeat_n(c, target_len));
            }
            3 => {
                // Strategy 4: Composite / Chaotic
                let components = rng.gen_range(2..20);
                for _ in 0..components {
                    match rng.gen_range(0..3) {
                        0 => self
                            .string_1
                            .extend_from_slice(MAGIC_PAYLOADS.choose(rng).unwrap().as_bytes()),
                        1 => {
                            if !self.wordlist.is_empty() {
                                self.string_1.extend_from_slice(
                                    self.wordlist.choose(rng).unwrap().as_bytes(),
                                )
                            }
                        }
                        _ => self
                            .string_1
                            .extend_from_slice(MAGIC_STRINGS.choose(rng).unwrap().as_bytes()),
                    }
                    self.string_1
                        .extend_from_slice(SEPARATORS.choose(rng).unwrap().as_bytes());
                }
            }
            4 => {
                // Strategy 5: Magic Payload
                self.string_1
                    .extend_from_slice(MAGIC_PAYLOADS.choose(rng).unwrap().as_bytes());
            }
            _ => {}
        }

        // Hard cap to prevent memory exhaustion
        if self.string_1.len() > 131072 {
            self.string_1.truncate(131072);
        }

        self.uri.clear();
        let uri_len_target = rng.gen_range(1..=128);

        match rng.gen_range(0..5) {
            0 => {
                // Strategy 0: Slash Flood
                let count = if uri_len_target >= 2 {
                    rng.gen_range(2..=uri_len_target).min(128)
                } else {
                    1
                };

                self.uri.extend(std::iter::repeat_n(b'/', count));

                if rng.gen_bool(0.3) && !self.wordlist.is_empty() {
                    self.uri
                        .extend_from_slice(self.wordlist.choose(rng).unwrap().as_bytes());
                }
            }
            1 => {
                // Strategy 1: Wordlist Extended
                self.uri.push(b'/');
                if !self.wordlist.is_empty() {
                    self.uri
                        .extend_from_slice(self.wordlist.choose(rng).unwrap().as_bytes());
                } else {
                    self.uri.extend_from_slice(b"index");
                }

                while self.uri.len() < uri_len_target {
                    match rng.gen_range(0..3) {
                        0 => self
                            .uri
                            .extend_from_slice(MAGIC_STRINGS.choose(rng).unwrap().as_bytes()),
                        1 => self
                            .uri
                            .extend_from_slice(SEPARATORS.choose(rng).unwrap().as_bytes()),
                        _ => {
                            let char_idx = rng.gen_range(0..PRINTABLES.len());
                            let c = PRINTABLES[char_idx];
                            self.uri.push(c);
                        }
                    }
                }
            }
            2 => {
                // Strategy 2: Magic Strings Only
                self.uri.push(b'/');
                while self.uri.len() < uri_len_target {
                    self.uri
                        .extend_from_slice(MAGIC_STRINGS.choose(rng).unwrap().as_bytes());
                }
            }
            3 => {
                // Strategy 3: Pure Chaos
                self.uri.push(b'/');
                for _ in 0..uri_len_target {
                    let char_idx = rng.gen_range(0..PRINTABLES.len());
                    let c = PRINTABLES[char_idx];
                    self.uri.push(c);
                }
            }
            4 => {
                // Strategy 4: Clean(ish)
                self.uri.push(b'/');
                if !self.wordlist.is_empty() {
                    self.uri
                        .extend_from_slice(self.wordlist.choose(rng).unwrap().as_bytes());
                }
            }
            _ => {}
        }

        if self.uri.len() > 128 {
            self.uri.truncate(128);
        }
        if self.uri.is_empty() {
            self.uri.push(b'/');
        }
    }
}
