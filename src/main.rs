use clap::Parser;

use copilot_money_cli::cli::Cli;

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    copilot_money_cli::cli::run(cli)
}
