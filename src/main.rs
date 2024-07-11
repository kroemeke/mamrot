use clap::Parser;
use rubik::Cube;
use rand::Rng;
use rand::seq::SliceRandom;

mod rubik;

#[derive(Parser, Debug)]
struct Args {
    #[arg(short, long)]
    target: String,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();


    let mut cube = Cube::new().load_headers("headers.txt")?;

    let mut request = String::new();
    request.push_str("GET / HTTP/1.1\r\n");
    request.push_str("Host: kroemeke.uk\r\n");
    
    // Initial throw of a dice...
    cube.rotate();


    let mut rnd = rand::thread_rng();

    // Some random headers from our list
    for _i in 1..cube.int_size_1 {
        request.push_str(&format!("{}: {}\r\n", cube.headers.choose(&mut rnd).expect("Whoopsie"), cube.string_1));
    }

    println!("{}",request);
    println!("\r\n");

    Ok(())
}
