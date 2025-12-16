use clap::Parser;

use copilot_money_api::cli::{Cli, OutputFormat};

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    match cli.command {
        copilot_money_api::cli::Command::Version => {
            println!("copilot-money-api (scaffold)");
            Ok(())
        }
        copilot_money_api::cli::Command::Hello => {
            let rows = vec![
                copilot_money_api::cli::KeyValueRow {
                    key: "status".to_string(),
                    value: "ok".to_string(),
                },
                copilot_money_api::cli::KeyValueRow {
                    key: "next".to_string(),
                    value: "capture GraphQL operations".to_string(),
                },
            ];
            match cli.output {
                OutputFormat::Json => {
                    let s = serde_json::to_string_pretty(&rows)?;
                    println!("{s}");
                    Ok(())
                }
                OutputFormat::Table => {
                    println!("{}", copilot_money_api::cli::render_table(rows));
                    Ok(())
                }
            }
        }
    }
}
