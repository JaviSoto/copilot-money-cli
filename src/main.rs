use clap::Parser;

use copilot_money_api::cli::Cli;

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    copilot_money_api::cli::run(cli)
}
