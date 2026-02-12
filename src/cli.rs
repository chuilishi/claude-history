use clap::Parser;

#[derive(Parser, Debug)]
#[command(name = "claude-history")]
#[command(about = "Resume Claude conversations")]
pub struct Args;
