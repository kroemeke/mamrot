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
pub static NGINX_METHODS: [&str; 16] = [
    "GET",
    "POST",
    "PUT",
    "DELETE",
    "HEAD",
    "OPTIONS",
    "TRACE",
    "CONNECT",
    "PATCH",
    "MKCOL",
    "COPY",
    "MOVE",
    "PROPFIND",
    "PROPPATCH",
    "LOCK",
    "UNLOCK",
];

// Python's string.printable
const PRINTABLES: &str = "0123456789abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ!\"#$%&'()*+,-./:;<=>?@[\\]^_`{|}~ \t\n\r\x0b\x0c";

#[derive(Debug, Clone)]
pub struct Cube {
    pub int_size_1: u64,
    pub _int_size_2: u64,
    _int_size_3: u64,
    _int_size_4: u64,
    _int_size_5: u64,
    int_size_magic: u64,
    pub string_1: String,
    pub uri: String,
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
            string_1: String::new(),
            uri: String::new(),
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
                // Strategy 1: Interleaved (Original Behavior modified slightly)
                // Limit the loop to avoid overly massive strings if int_size_magic is huge,
                // or just rely on int_size_magic as the total count.
                // We'll use int_size_magic as a rough length target or iteration count.
                // Since MAGIC_NUMBERS go up to 65535, we should be careful not to make it length * len(word).
                // Let's iterate up to int_size_magic / 10 + 1 to keep it sane but "brave".
                let iterations = (self.int_size_magic as usize / 10).max(1);
                for _ in 0..iterations {
                    self.string_1.push_str(
                        MAGIC_STRINGS
                            .choose(rng)
                            .expect("Internal Error: MAGIC_STRINGS empty"),
                    );
                    if !self.wordlist.is_empty() {
                        self.string_1.push_str(
                            self.wordlist
                                .choose(rng)
                                .expect("Internal Error: Wordlist empty during rotation"),
                        );
                    }
                }
            }
            1 => {
                // Strategy 2: Pure Repetition (Magic String * N)
                let magic = MAGIC_STRINGS.choose(rng).unwrap();
                // If magic is long, we might want fewer repetitions.
                // But "brave" means long.
                for _ in 0..self.int_size_magic {
                    self.string_1.push_str(magic);
                }
            }
            2 => {
                // Strategy 3: Character Flood (Random Char * (N + small offset))
                let char_idx = rng.gen_range(0..PRINTABLES.len());
                // Rust char slicing on ASCII/UTF-8 string needs care, but PRINTABLES is all 1-byte chars here except maybe control codes.
                // safer to just pick a char.
                let c = PRINTABLES.chars().nth(char_idx).unwrap_or('A');

                // Fuzz slightly beyond the magic number to trigger off-by-one or small buffer overflows
                // e.g., if Magic is 256, we might send 257, 260, 266...
                let offset = rng.gen_range(0..=32);
                let target_len = self.int_size_magic + offset;

                for _ in 0..target_len {
                    self.string_1.push(c);
                }
            }
            3 => {
                // Strategy 4: Composite / Chaotic
                // Mimic Archer: v += random.choice(rubik.cube) + random.choice(http_separators) ...
                let components = rng.gen_range(2..20);
                for _ in 0..components {
                    match rng.gen_range(0..3) {
                        0 => self.string_1.push_str(MAGIC_PAYLOADS.choose(rng).unwrap()),
                        1 => {
                            if !self.wordlist.is_empty() {
                                self.string_1.push_str(self.wordlist.choose(rng).unwrap())
                            }
                        }
                        _ => self.string_1.push_str(MAGIC_STRINGS.choose(rng).unwrap()),
                    }
                    self.string_1.push_str(SEPARATORS.choose(rng).unwrap());
                }
            }
            4 => {
                // Strategy 5: Magic Payload (Single edge case integer)
                self.string_1.push_str(MAGIC_PAYLOADS.choose(rng).unwrap());
            }
            _ => {}
        }

        // Hard cap to prevent memory exhaustion / excessive packet size if logic goes wild
        if self.string_1.len() > 131072 {
            // 128KB cap
            self.string_1.truncate(131072);
        }

        self.uri.clear();
        let uri_len_target = rng.gen_range(1..=128);

        match rng.gen_range(0..5) {
            0 => {
                // Strategy 0: Slash Flood / Path Traversal simulation
                // e.g. //////////// or /////WORD
                let count = if uri_len_target >= 2 {
                    rng.gen_range(2..=uri_len_target).min(128)
                } else {
                    1
                };
                for _ in 0..count {
                    self.uri.push('/');
                }
                // Occasionally append a word
                if rng.gen_bool(0.3) && !self.wordlist.is_empty() {
                    self.uri.push_str(self.wordlist.choose(rng).unwrap());
                }
            }
            1 => {
                // Strategy 1: Wordlist Extended (WORD%n%n...)
                self.uri.push('/');
                if !self.wordlist.is_empty() {
                    self.uri.push_str(self.wordlist.choose(rng).unwrap());
                } else {
                    self.uri.push_str("index");
                }

                // Append junk (separators, magic, chars) until target length
                while self.uri.len() < uri_len_target {
                    match rng.gen_range(0..3) {
                        0 => self.uri.push_str(MAGIC_STRINGS.choose(rng).unwrap()),
                        1 => self.uri.push_str(SEPARATORS.choose(rng).unwrap()),
                        _ => {
                            let char_idx = rng.gen_range(0..PRINTABLES.len());
                            let c = PRINTABLES.chars().nth(char_idx).unwrap_or('a');
                            self.uri.push(c);
                        }
                    }
                }
            }
            2 => {
                // Strategy 2: Magic Strings Only (starting with /)
                self.uri.push('/');
                while self.uri.len() < uri_len_target {
                    self.uri.push_str(MAGIC_STRINGS.choose(rng).unwrap());
                }
            }
            3 => {
                // Strategy 3: Pure Chaos (random printable, starts with /)
                self.uri.push('/');
                for _ in 0..uri_len_target {
                    let char_idx = rng.gen_range(0..PRINTABLES.len());
                    let c = PRINTABLES.chars().nth(char_idx).unwrap_or('a');
                    self.uri.push(c);
                }
            }
            4 => {
                // Strategy 4: Clean(ish) - just a word
                self.uri.push('/');
                if !self.wordlist.is_empty() {
                    self.uri.push_str(self.wordlist.choose(rng).unwrap());
                }
            }
            _ => {}
        }

        // Hard cap to 128 bytes
        if self.uri.len() > 128 {
            self.uri.truncate(128);
        }
        // Ensure it's not empty
        if self.uri.is_empty() {
            self.uri.push('/');
        }
    }
}
