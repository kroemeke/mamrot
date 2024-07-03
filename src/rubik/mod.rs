use rand::Rng;

// Typical buffer sizes
static magic_numbers: [u64; 11] = [32,64,128,256,512,1024,2048,4096,8192,32767,65535];

// Format strings and header value separators
static magic_strings: [&str; 5] = ["%s%n%x", "%", ",", " ", "."];

#[derive(Debug, Default)]
pub struct Cube {
    int_size_1: u64,
    int_size_2: u64,
    int_size_3: u64,
    int_size_4: u64,
    int_size_5: u64,
    int_size_magic: u64,
}

impl Cube {
    pub fn new() -> Cube {
        let mut rng = rand::thread_rng();
        Cube {
            int_size_1: rng.gen_range(1..=10),
            int_size_2: rng.gen_range(1..=1000),
            int_size_3: rng.gen_range(1..=100000),
            int_size_4: rng.gen_range(1..=10000000),
            int_size_5: rng.gen_range(1..=1000000000),
            ..Default::default()
        }
    }
}
