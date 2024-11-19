use clap::Parser;
use rand::seq::SliceRandom;
use rand::Rng;
use rubik::Cube;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;

mod rubik;

#[derive(Parser, Debug)]
struct Args {
    #[arg(short, long)]
    target: String,

    #[arg(long, default_value = "headers.txt")]
    headers: String,

    #[arg(short, long, default_value = "wordlist.txt")]
    wordlist: String,

    #[arg(short, long, default_value_t = 80)]
    port: usize,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();

    let mut cube = Cube::new()
        .load_headers(&args.headers)?
        .load_wordlist(&args.wordlist)?;

    loop {
        let mut request = String::new();
        for n in 1..3 {
        request.push_str("GET / HTTP/1.1\r\nHost: ");
        request.push_str(&args.target);
        request.push_str("\r\n");

        // Initial throw of a dice...
        cube.rotate();

        let mut rnd = rand::thread_rng();

        // Some random headers from our list
        for _i in 1..cube.int_size_1 {
            request.push_str(&format!(
                "{}: {}\r\n",
                cube.headers.choose(&mut rnd).expect("Whoopsie"),
                cube.string_1
            ));
            cube.rotate();
        }

        // Final \r\n
        request.push_str("\r\n");
        };

        // XXX
        match TcpStream::connect(format!("{}:{}", &args.target, &args.port)).await {
            Ok(mut stream) => {
                // Manually construct the HTTP GET request
                // Send the request
                if let Err(e) = stream.write_all(request.as_bytes()).await {
                    eprintln!("Failed to send request: {}", e);
                    continue;
                }

                // Read the response
                let mut response = Vec::new();
                if let Err(e) = stream.read_to_end(&mut response).await {
                    eprintln!("Failed to read response: {}", e);
                    continue;
                }

                // Optionally, print the response status line
                if let Some(status_line) = String::from_utf8_lossy(&response).lines().next() {
                    println!("Received: {}", status_line);
                }
            }
            Err(e) => {
                eprintln!("Failed to connect: {}", e);
            }
        }
        // XXX
    }

    Ok(())
}
