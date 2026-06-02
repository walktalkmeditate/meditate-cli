use clap::Parser;
use meditate::cli::Cli;

fn main() {
    std::process::exit(meditate::run(Cli::parse()));
}
