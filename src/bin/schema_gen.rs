use std::fs;
use std::path::PathBuf;

use clap::Parser;

#[derive(Debug, Parser)]
#[command(name = "schema-gen")]
#[command(about = "Generate a best-effort schema stub from GraphQL operations")]
struct Args {
    /// Directory containing `.graphql` documents.
    #[arg(long, default_value = "graphql")]
    graphql_dir: PathBuf,

    #[arg(long, default_value = "schema/schema.graphql")]
    out: PathBuf,
}

fn main() -> anyhow::Result<()> {
    let args = Args::parse();

    let graphql_dir = args.graphql_dir;

    let mut docs = Vec::new();
    for entry in fs::read_dir(&graphql_dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.extension().and_then(|s| s.to_str()) != Some("graphql") {
            continue;
        }
        docs.push(path);
    }
    docs.sort();

    let content = copilot_money_cli::schema_gen::render_schema_from_operations(&docs)?;
    if let Some(parent) = args.out.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(&args.out, content)?;
    Ok(())
}
