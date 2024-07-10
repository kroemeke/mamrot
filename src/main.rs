use clap::Parser;
use rubik::Cube;

mod rubik;

#[derive(Parser, Debug)]
struct Args {
    #[arg(short, long)]
    target: String,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();

    println!("Hello, {}!", args.target);

    let mut cube = Cube::new().load_headers("headers.txt")?;
    dbg!(&cube);
    for _i in 1..6 {
        cube.rotate();
        dbg!(&cube);
    }

    Ok(())
}
