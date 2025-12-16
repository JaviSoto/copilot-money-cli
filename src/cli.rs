use clap::{Parser, Subcommand, ValueEnum};
use serde::Serialize;
use tabled::Table;

#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum OutputFormat {
    Json,
    Table,
}

#[derive(Debug, Parser)]
#[command(name = "copilot")]
#[command(
    about = "CLI for Copilot Money (unofficial)",
    version,
    disable_version_flag = true
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,

    #[arg(long, value_enum, default_value_t = OutputFormat::Table, global = true)]
    pub output: OutputFormat,
}

#[derive(Debug, Subcommand)]
pub enum Command {
    /// Print version info
    Version,

    /// Sanity check output modes
    #[command(hide = true)]
    Hello,
}

#[derive(Debug, Clone, Serialize, tabled::Tabled)]
pub struct KeyValueRow {
    pub key: String,
    pub value: String,
}

pub fn render_table(rows: Vec<KeyValueRow>) -> String {
    Table::new(rows).to_string()
}
