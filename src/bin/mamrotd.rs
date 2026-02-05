use clap::Parser;
use mamrot::rubik::Cube;
use mamrot::seed;
use rand::rngs::SmallRng;
use rand::seq::SliceRandom;
use rand::{Rng, SeedableRng};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;
use tokio::time::{self, Duration};

#[derive(Parser, Debug, Clone)]
struct Args {
    #[arg(short, long, default_value_t = 8100)]
    port: usize,

    #[arg(long, default_value = "response_headers.txt")]
    headers: String,

    #[arg(short, long, default_value = "wordlist.txt")]
    wordlist: String,

    #[arg(long, default_value = "seeds.bin")]
    seed_log: String,

    #[arg(long, default_value_t = 1_000_000)]
    seed_buffer_size: usize,

    #[arg(long)]
    replay_file: Option<String>,

    #[arg(long, default_value_t = 10)]
    timeout: u64,
}

// Mimicking archerd/modules/httpd values
static HTTP_VERSIONS: [&str; 5] = ["0.9", "1.0", "1.1", "2.0", "3.0"];

static HTTP_CODES: [&str; 37] = [
    "100", "101", "200", "201", "202", "203", "204", "205", "206", "300", "301", "302", "303",
    "304", "305", "307", "400", "401", "403", "404", "405", "406", "407", "408", "409", "410",
    "411", "412", "413", "414", "416", "500", "501", "502", "503", "504", "505",
];

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Arc::new(Args::parse());

    eprintln!("Starting mamrotd on port {}", args.port);

    // Load initial cube state
    let base_cube = Cube::new()
        .load_headers(&args.headers)?
        .load_wordlist(&args.wordlist)?
        // Use a default max header count similar to client, or hardcode if needed.
        // Archer uses rubik.rubik_int_r0 which is rand(0,10)
        .with_max_headers(10);

    // --- SEED STRATEGY SETUP ---
    let mut seed_log: Option<Arc<seed::SeedLog>> = None;
    let mut replay_seeds: Option<Arc<Vec<u64>>> = None;
    let mut replay_index: Option<Arc<AtomicUsize>> = None;

    if let Some(file_path) = &args.replay_file {
        eprintln!("Loading seeds for replay from: {}", file_path);
        let seeds = seed::load_seeds(file_path)?;
        eprintln!("Loaded {} seeds for infinite replay loop.", seeds.len());
        replay_seeds = Some(Arc::new(seeds));
        replay_index = Some(Arc::new(AtomicUsize::new(0)));
    } else {
        // Normal Fuzzing Mode: Setup Ring Buffer Logger
        let logger = seed::SeedLog::new(&args.seed_log, args.seed_buffer_size)?;
        seed_log = Some(Arc::new(logger));
    }

    // Bind Listener
    let addr = format!("0.0.0.0:{}", args.port);
    let listener = TcpListener::bind(&addr).await?;

    eprintln!("Listening on {}", addr);

    loop {
        let (mut socket, _peer_addr) = listener.accept().await?;
        
        let args = args.clone();
        let mut cube = base_cube.clone();
        let seed_log = seed_log.clone();
        let replay_seeds = replay_seeds.clone();
        let replay_index = replay_index.clone();

        tokio::spawn(async move {
            // Optional: Set timeout for the whole interaction
            let timeout = Duration::from_secs(5);
            let _ = time::timeout(timeout, async move {
                // 1. Read / Wait for data
                // Archer uses twisted's dataReceived. We just read up to some bytes into a buffer
                // We don't really care what they sent, but we wait for *something* to trigger response
                let mut buf = [0u8; 1024];
                match socket.read(&mut buf).await {
                    Ok(0) => return, // EOF
                    Ok(_) => {} // Got data, proceed
                    Err(_) => return, // Error
                };

                // 2. Determine Seed
                let seed = if let Some(seeds) = &replay_seeds {
                    let idx = replay_index
                        .as_ref()
                        .unwrap()
                        .fetch_add(1, Ordering::Relaxed);
                    seeds[idx % seeds.len()]
                } else {
                    let mut rng = SmallRng::from_entropy();
                    let s = rng.gen::<u64>();
                    if let Some(logger) = &seed_log {
                        logger.log(s);
                    }
                    s
                };

                // 3. Fuzz
                let mut rng = SmallRng::seed_from_u64(seed);
                
                // Rotate cube once to set initial random state
                cube.rotate(&mut rng);

                // 4. Construct Response
                let version = HTTP_VERSIONS.choose(&mut rng).unwrap();
                let code = HTTP_CODES.choose(&mut rng).unwrap();
                
                let mut response = Vec::with_capacity(4096);
                
                // Status Line
                response.extend_from_slice(b"HTTP/");
                response.extend_from_slice(version.as_bytes());
                response.extend_from_slice(b" ");
                response.extend_from_slice(code.as_bytes());
                response.extend_from_slice(b" OK\r\n");

                // Headers
                // Archer: for header in range(0, rubik.rubik_int_r0):
                // rubik_int_r0 is randomly set during rotate (mapped to int_size_1 in Cube)
                for _ in 0..cube.int_size_1 {
                    let default_header = String::from("X-Fuzz");
                    let header_name = cube.headers.choose(&mut rng).unwrap_or(&default_header);
                    
                    // Generate Value
                    // Archer: v += random.choice(rubik.cube) + random.choice(http_separators) + random.choice(rubik.cube)
                    // We will approximate this using cube.string_1 which is populated with similar garbage
                    // But to be closer to archer's loop:
                    //   for value in range(0, rubik.rubik_int_r0):
                    //     v += ...
                    
                    // In our Cube::rotate, string_1 is already built using complex strategies.
                    // We will just use string_1 as the value.
                    // If we want to strictly mimic the "header value composed of multiple cube items separated by separators",
                    // we might need to do that manually here, but reusing `cube.string_1` is safer/cleaner given the instruction
                    // "Don't insist on using rubik if it would mean changes to mamrot http client".
                    // The current `rotate` strategies in `mod.rs` already produce complex strings including separators.
                    
                    response.extend_from_slice(header_name.as_bytes());
                    response.extend_from_slice(b": ");
                    response.extend_from_slice(&cube.string_1);
                    response.extend_from_slice(b"\r\n");

                    // Rotate for next header
                    cube.rotate(&mut rng);
                }

                response.extend_from_slice(b"\r\n\r\n");
                response.extend_from_slice(b"A\r\n"); // Body from archerd

                // 5. Write and Close
                let _ = socket.write_all(&response).await;

                if args.timeout > 0 {
                    time::sleep(Duration::from_millis(args.timeout)).await;
                }
                // Connection drops when socket goes out of scope
            }).await;
        });
    }
}
