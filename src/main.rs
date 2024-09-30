#![feature(backtrace)]
#![feature(box_patterns)]
#![feature(rustc_private)]
#![feature(try_blocks)]
#![feature(never_type)]

mod logger;
mod options;
mod rudra;

use anyhow::Context;
use charon_lib::ast::TranslatedCrate;
use charon_lib::export::CrateData;
use clap::Parser;
use options::*;
//use std::collections::{HashMap, HashSet};
//use std::fmt::Write;
//use std::fs;
use std::fs::File;
use std::io::BufReader;
//use std::path::PathBuf;
//use std::process::Command;

fn main() {
    // Initialize the logger
    //logger::initialize_logger();
    let _ = rudra::log::setup_logging(rudra::log::Verbosity::Normal).unwrap();

    // Parse the command-line
    let options = CliOpts::parse();

    // Deserialize the .ullbc file
    let crate_data: TranslatedCrate = {
        use serde::Deserialize;
        let file = File::open(&options.file)
            .with_context(|| format!("Failed to read ullbc file {}", &options.file))
            .unwrap();
        let reader = BufReader::new(file);
        let mut deserializer = serde_json::Deserializer::from_reader(reader);
        // Deserialize without recursion limit.
        deserializer.disable_recursion_limit();
        // Grow stack space as needed.
        let deserializer = serde_stacker::Deserializer::new(&mut deserializer);
        CrateData::deserialize(deserializer)
            .expect("Could not deserialize the ullbc file")
            .translated
    };

    let config = crate::rudra::lib::RudraConfig::default();
    crate::rudra::lib::analyze(crate_data, config);
}
