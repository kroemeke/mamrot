use clap::Parser;
use mamrot::rubik::Cube;
use mamrot::seed;
use rand::rngs::SmallRng;
use rand::seq::SliceRandom;
use rand::{Rng, SeedableRng};
use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, RwLock};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::TcpStream;
use tokio::sync::mpsc;
use tokio::time::{self, Duration};

#[derive(Parser, Debug, Clone)]
struct Args {
    #[arg(short, long)]
    target: String,

    #[arg(long, default_value = "request_headers.txt")]
    headers: String,

    #[arg(short, long, default_value = "wordlist.txt")]
    wordlist: String,

    #[arg(short, long, default_value_t = 80)]
    port: usize,

    #[arg(long, default_value_t = 120)]
    timeout: u64,

    #[arg(short, long, default_value_t = 600)]
    concurrency: usize,

    #[arg(long, default_value_t = 2)]
    max_headers: u64,

    #[arg(long, default_value_t = 2)]
    max_lag: usize,

    #[arg(long, default_value_t = 4)]
    batch_size: usize,

    #[arg(long, default_value = "seeds.bin")]
    seed_log: String,

    #[arg(long, default_value_t = 1_000_000)]
    seed_buffer_size: usize,

    #[arg(long, default_value_t = 30)]
    dns_refresh: u64,

    #[arg(long)]
    replay: Option<u64>,

    #[arg(long)]
    replay_file: Option<String>,
}

enum Event {
    Attempted(usize),
    Sent(usize),
    BytesSent(usize),
    Status(String),
    Error(String),
    LagPaused,
    LagTimeout,
    ConnectionClosed(u64),
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Arc::new(Args::parse());

    // Load initial cube state
    let mut base_cube = Cube::new()
        .load_headers(&args.headers)?
        .load_wordlist(&args.wordlist)?
        .with_max_headers(args.max_headers);

    // --- REPLAY MODE ---
    if let Some(seed) = args.replay {
        let mut rng = SmallRng::seed_from_u64(seed);
        // Using Vec<u8> for request buffer to match new Cube structure
        let mut request_buf = Vec::with_capacity(4096);
        let req_host = args.target.as_bytes();
        let req_end = b"\r\n";

        eprintln!("Replaying batch with seed: {}", seed);

        for _ in 0..args.batch_size {
            base_cube.rotate(&mut rng);

            // Construct Request Line
            let method = base_cube
                .methods
                .choose(&mut rng)
                .expect("Internal Error: Methods empty");
            request_buf.extend_from_slice(method.as_bytes());
            request_buf.push(b' ');

            if !base_cube.uri.starts_with(b"/") && !base_cube.uri.starts_with(b"http") {
                request_buf.push(b'/');
            }
            request_buf.extend_from_slice(&base_cube.uri);
            request_buf.extend_from_slice(b" HTTP/1.1\r\nHost: ");
            request_buf.extend_from_slice(req_host);
            request_buf.extend_from_slice(req_end);

            for _ in 0..base_cube.int_size_1 {
                let header_name = base_cube
                    .headers
                    .choose(&mut rng)
                    .expect("Internal Error: Headers empty");

                request_buf.extend_from_slice(header_name.as_bytes());
                request_buf.extend_from_slice(b": ");
                request_buf.extend_from_slice(&base_cube.string_1);
                request_buf.extend_from_slice(b"\r\n");

                base_cube.rotate(&mut rng);
            }
            request_buf.extend_from_slice(b"\r\n");

            // Print request delimiter for clarity if multiple in batch
            println!("{}", String::from_utf8_lossy(&request_buf));
            request_buf.clear();
        }
        return Ok(());
    }

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

    // --- DNS RESOLUTION & REFRESH ---
    let target_host = format!("{}:{}", args.target, args.port);
    let initial_ips: Vec<SocketAddr> = tokio::net::lookup_host(&target_host)
        .await
        .map_err(|e| format!("Failed to resolve {}: {}", target_host, e))?
        .collect();

    if initial_ips.is_empty() {
        return Err("DNS resolution returned no IPs".into());
    }

    eprintln!("Target resolved to: {:?}", initial_ips);
    let target_ips = Arc::new(RwLock::new(initial_ips));

    // Spawn DNS Refresh Task
    {
        let target_ips = target_ips.clone();
        let target_host = target_host.clone();
        let refresh_interval = Duration::from_secs(args.dns_refresh);
        tokio::spawn(async move {
            loop {
                time::sleep(refresh_interval).await;
                match tokio::net::lookup_host(&target_host).await {
                    Ok(iter) => {
                        let new_ips: Vec<SocketAddr> = iter.collect();
                        if !new_ips.is_empty() {
                            if let Ok(mut lock) = target_ips.write() {
                                *lock = new_ips;
                            }
                        }
                    }
                    Err(_) => {
                        // Keep using old IPs on failure
                    }
                }
            }
        });
    }

    let (tx, mut rx) = mpsc::channel::<Event>(1024 * 10); // Buffer for events

    // Stats Printer Task
    tokio::spawn(async move {
        let mut stats: HashMap<String, u64> = HashMap::new();
        let mut total_attempted: u64 = 0;
        let mut total_sent: u64 = 0;
        let mut total_bytes: u64 = 0;
        let mut total_responses: u64 = 0;
        let mut total_paused: u64 = 0;
        let mut total_timeouts: u64 = 0;

        // Tracking connection stats
        let mut total_closed_connections: u64 = 0;
        let mut total_requests_on_closed: u64 = 0;

        let mut last_attempted: u64 = 0;
        let mut last_sent: u64 = 0;
        let mut last_bytes: u64 = 0;
        let mut last_responses: u64 = 0;

        let mut interval = time::interval(Duration::from_secs(1));
        interval.tick().await; // First tick is immediate

        loop {
            tokio::select! {
                Some(event) = rx.recv() => {
                    match event {
                        Event::Attempted(count) => total_attempted += count as u64,
                        Event::Sent(count) => total_sent += count as u64,
                        Event::BytesSent(bytes) => total_bytes += bytes as u64,
                        Event::Status(code) => {
                            *stats.entry(code).or_insert(0) += 1;
                            total_responses += 1;
                        }
                        Event::Error(err) => {
                            *stats.entry(err).or_insert(0) += 1;
                        }
                        Event::LagPaused => total_paused += 1,
                        Event::LagTimeout => total_timeouts += 1,
                        Event::ConnectionClosed(req_count) => {
                            total_closed_connections += 1;
                            total_requests_on_closed += req_count;
                        }
                    }
                }
                _ = interval.tick() => {
                    let attempt_delta = total_attempted - last_attempted;
                    let sent_delta = total_sent - last_sent;
                    let bytes_delta = total_bytes - last_bytes;
                    let resp_delta = total_responses - last_responses;

                    let attempt_rps = attempt_delta as f64;
                    let sent_rps = sent_delta as f64;
                    let mbps = (bytes_delta as f64 * 8.0) / (1024.0 * 1024.0);
                    let resp_rps = resp_delta as f64;

                    let avg_req_per_conn = if total_closed_connections > 0 {
                        total_requests_on_closed as f64 / total_closed_connections as f64
                    } else {
                        0.0
                    };

                    last_attempted = total_attempted;
                    last_sent = total_sent;
                    last_bytes = total_bytes;
                    last_responses = total_responses;

                    if !stats.is_empty() || sent_delta > 0 || attempt_delta > 0 {
                        println!("Pauses: {}, Timeouts: {}", total_paused, total_timeouts);
                        let mut sorted_stats: Vec<_> = stats.iter().collect();
                        sorted_stats.sort_by(|a, b| b.1.cmp(a.1));

                        let (statuses, errors): (Vec<_>, Vec<_>) = sorted_stats
                            .into_iter()
                            .partition(|(k, _)| k.chars().all(char::is_numeric));

                        for (err, count) in errors {
                            println!("{}: {} times", err, count);
                        }

                        if !statuses.is_empty() {
                            let mut status_line = String::from("Status: ");
                            for (code, count) in statuses {
                                use std::fmt::Write;
                                let _ = write!(status_line, "{}[{}] ", code, count);
                            }
                            println!("{}", status_line);
                        }
                        println!("--- Stats (RPS: Attempt {:.2}, Sent {:.2}, Resp {:.2}) | BW: {:.2} Mbps | Avg Req/Conn: {:.2} ---", attempt_rps, sent_rps, resp_rps, mbps, avg_req_per_conn);
                        println!("-------------");
                    }
                }
            }
        }
    });

    // Spawn Workers
    for _ in 0..args.concurrency {
        let args = args.clone();
        let mut cube = base_cube.clone();
        let tx = tx.clone();

        let seed_log = seed_log.clone();
        let replay_seeds = replay_seeds.clone();
        let replay_index = replay_index.clone();
        let target_ips = target_ips.clone();

        tokio::spawn(async move {
            let mut request_buf = Vec::with_capacity(4096);
            let mut master_rng = SmallRng::from_entropy(); // Use SmallRng for speed

            // Worker Loop (Reconnects on failure/lag)
            loop {
                // Connect
                let target_addr = {
                    let ips = target_ips.read().unwrap();
                    *ips.choose(&mut master_rng).expect("IP list empty")
                };

                let stream = match TcpStream::connect(target_addr).await {
                    Ok(s) => s,
                    Err(_) => {
                        let _ = tx.send(Event::Error("Connect Error".to_string())).await;
                        time::sleep(Duration::from_secs(1)).await;
                        continue;
                    }
                };

                let mut requests_sent_this_conn: u64 = 0;

                let _ = stream.set_nodelay(true);

                let (rd, mut wr) = stream.into_split();
                let lag = Arc::new(AtomicUsize::new(0));
                let stop_signal = Arc::new(AtomicBool::new(false));

                // Reader Task
                let lag_reader = lag.clone();
                let stop_reader = stop_signal.clone();
                let tx_reader = tx.clone();

                // We spawned the reader logic in a separate task so the writer can keep spinning
                let reader_handle = tokio::spawn(async move {
                    let mut reader = BufReader::new(rd);
                    let mut line = String::new();

                    loop {
                        line.clear();
                        match reader.read_line(&mut line).await {
                            Ok(0) => {
                                // EOF
                                stop_reader.store(true, Ordering::Relaxed);
                                break;
                            }
                            Ok(_) => {
                                // Simple HTTP/1.1 check
                                if line.starts_with("HTTP/1.1 ") || line.starts_with("HTTP/1.0 ") {
                                    let parts: Vec<&str> = line.split_whitespace().collect();
                                    if parts.len() >= 2 {
                                        let code = parts[1].to_string();
                                        let _ = tx_reader.send(Event::Status(code)).await;
                                        // We got a response, decrement lag
                                        lag_reader.fetch_sub(1, Ordering::Relaxed);
                                    }
                                }
                            }
                            Err(_) => {
                                // Read Error
                                let _ =
                                    tx_reader.send(Event::Error("Read Error".to_string())).await;
                                stop_reader.store(true, Ordering::Relaxed);
                                break;
                            }
                        }
                    }
                });

                // Writer Loop
                // Pre-calculate fixed parts
                let req_host = args.target.as_bytes();
                let req_end = b"\r\n";

                loop {
                    if stop_signal.load(Ordering::Relaxed) {
                        break;
                    }

                    let current_lag = lag.load(Ordering::Relaxed);
                    // Check lag against limit - batch_size (allow at least one batch if below max_lag)
                    if current_lag > args.max_lag {
                        let _ = tx.send(Event::LagPaused).await;
                        let mut last_lag = current_lag;
                        let mut last_activity = time::Instant::now();
                        let mut timed_out = false;

                        while lag.load(Ordering::Relaxed) > args.max_lag {
                            if last_activity.elapsed().as_millis() as u64 > args.timeout {
                                timed_out = true;
                                break;
                            }

                            // Check if we received anything (lag decreased)
                            let new_lag = lag.load(Ordering::Relaxed);
                            if new_lag < last_lag {
                                last_lag = new_lag;
                                last_activity = time::Instant::now(); // Reset timeout
                            }

                            time::sleep(Duration::from_millis(10)).await;
                        }

                        if timed_out {
                            let _ = tx.send(Event::LagTimeout).await;
                            break;
                        }
                    }

                    // Prepare Batch
                    request_buf.clear();

                    // --- DETERMINE SEED ---
                    let seed = if let Some(seeds) = &replay_seeds {
                        // REPLAY MODE: Fetch next seed from list (looping)
                        let idx = replay_index
                            .as_ref()
                            .unwrap()
                            .fetch_add(1, Ordering::Relaxed);
                        seeds[idx % seeds.len()]
                    } else {
                        // FUZZ MODE: Generate new seed and log it
                        let s = master_rng.gen::<u64>();
                        if let Some(logger) = &seed_log {
                            logger.log(s);
                        }
                        s
                    };

                    let mut batch_rng = SmallRng::seed_from_u64(seed);

                    for _ in 0..args.batch_size {
                        cube.rotate(&mut batch_rng);

                        // Construct Request Line
                        // Method
                        let method = cube
                            .methods
                            .choose(&mut batch_rng)
                            .expect("Internal Error: Methods empty");
                        request_buf.extend_from_slice(method.as_bytes());
                        request_buf.push(b' ');

                        // URI
                        if !cube.uri.starts_with(b"/") && !cube.uri.starts_with(b"http") {
                            request_buf.push(b'/');
                        }
                        request_buf.extend_from_slice(&cube.uri);

                        request_buf.extend_from_slice(b" HTTP/1.1\r\nHost: ");
                        request_buf.extend_from_slice(req_host);
                        request_buf.extend_from_slice(req_end);

                        for _ in 0..cube.int_size_1 {
                            // Using push_str to avoid format! allocation
                            let header_name = cube
                                .headers
                                .choose(&mut batch_rng)
                                .expect("Internal Error: Headers empty");

                            request_buf.extend_from_slice(header_name.as_bytes());
                            request_buf.extend_from_slice(b": ");
                            request_buf.extend_from_slice(&cube.string_1);
                            request_buf.extend_from_slice(b"\r\n");

                            cube.rotate(&mut batch_rng);
                        }
                        request_buf.extend_from_slice(b"\r\n");
                    }

                    // Write with Timeout
                    // Use a generous timeout for writes to allow TCP buffers to drain on saturated links.
                    // If args.timeout is very low (e.g. user set 10ms), we enforce a 5s minimum for writes
                    // to prevent thrashing.
                    let write_timeout = Duration::from_millis(args.timeout.max(5000));

                    let _ = tx.send(Event::Attempted(args.batch_size)).await;

                    let write_result =
                        time::timeout(write_timeout, wr.write_all(&request_buf)).await;

                    match write_result {
                        Ok(Ok(())) => {
                            // Success
                            let _ = tx.send(Event::Sent(args.batch_size)).await;
                            let _ = tx.send(Event::BytesSent(request_buf.len())).await;
                            lag.fetch_add(args.batch_size, Ordering::Relaxed);
                            requests_sent_this_conn += args.batch_size as u64;
                        }
                        Ok(Err(e)) => {
                            let _ = tx.send(Event::Error(format!("Write Error: {}", e))).await;
                            // Even on error, we might have sent bytes.
                            // But write_all error doesn't tell us how many easily without custom loop.
                            // For now, we assume 0 for stats correctness on "Sent",
                            // but the user should know "Attempted" was high.
                            break;
                        }
                        Err(_) => {
                            let _ = tx.send(Event::Error("Write Timeout".to_string())).await;
                            break;
                        }
                    }
                }

                // Cleanup: Writer loop broken (lag or stop signal)
                // Report stats for this connection
                let _ = tx
                    .send(Event::ConnectionClosed(requests_sent_this_conn))
                    .await;

                // We drop `wr`, which closes the write side of the socket.
                // We should also abort the reader if it's still stuck waiting for data.
                reader_handle.abort();
            }
        });
    }

    // Keep main alive
    loop {
        time::sleep(Duration::from_secs(60)).await;
    }
}
