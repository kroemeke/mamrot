use clap::Parser;
use rubik::Cube;

mod rubik;


#[derive(Parser, Debug)]
struct Args {
    #[arg(short, long)]
    target: String,
}

fn main() {
    let args = Args::parse();

    println!("Hello, {}!", args.target);

    let cube = Cube::new();
    dbg!(cube);
}
