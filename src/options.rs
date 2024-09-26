use clap::Parser;

#[derive(Debug, Default, Clone, Parser)]
#[clap(name = "CharonRudra")]
pub struct CliOpts {
    #[clap(long = "file")]
    pub file: String,
}
