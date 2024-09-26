mod options;
mod logger;


use options::*;
use clap::Parser;



fn main() {
    // Initialize the logger
    logger::initialize_logger();

    // Parse the command-line
    let mut options = CliOpts::parse();
}
