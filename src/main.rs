mod context;
mod logger;
mod options;
mod rudra;

use anyhow::{bail, Context, Result};
use charon_lib::ast::{AttrInfo, Attribute, TranslatedCrate};
use charon_lib::export::CrateData;
use charon_lib::meta::ItemMeta;
use charon_lib::names::{Name, PathElem};
use charon_lib::types::{
    BuiltinTy, Field, LiteralTy, Ty, TypeDecl, TypeDeclId, TypeDeclKind, TypeId,
};
use clap::Parser;
use options::*;
use std::collections::{HashMap, HashSet};
use std::fmt::Write;
use std::fs;
use std::fs::File;
use std::io::BufReader;
use std::path::PathBuf;
use std::process::Command;

fn main() {
    // Initialize the logger
    logger::initialize_logger();

    // Parse the command-line
    let mut options = CliOpts::parse();

    // Deserialize the .ullbc file
    let crate_data: TranslatedCrate = {
        use serde::Deserialize;
        let file = File::open(&options.file)
            .with_context(|| format!("Failed to read llbc file {}", &options.file))
            .unwrap();
        let reader = BufReader::new(file);
        let mut deserializer = serde_json::Deserializer::from_reader(reader);
        // Deserialize without recursion limit.
        deserializer.disable_recursion_limit();
        // Grow stack space as needed.
        let deserializer = serde_stacker::Deserializer::new(&mut deserializer);
        CrateData::deserialize(deserializer)
            .expect("Could not deserialize the llbc file")
            .translated
    };

    let ctx = context::Ctx::new(crate_data);
}
