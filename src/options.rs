use clap::Parser;
use serde::{Deserialize, Serialize};

#[derive(Debug, Default, Clone, Parser, Serialize, Deserialize)]
#[clap(name = "CharonRudra")]
pub struct CliOpts {
    #[clap(long = "file")]
    #[serde(default)]
    pub bin: String,
}
