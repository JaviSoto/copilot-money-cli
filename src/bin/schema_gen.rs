use std::fs;
use std::path::PathBuf;

use clap::Parser;

#[derive(Debug, Parser)]
#[command(name = "schema-gen")]
#[command(about = "Generate a best-effort schema stub from GraphQL operations")]
struct Args {
    /// Directory containing `.graphql` documents (defaults to newest under `artifacts/graphql-ops/*/graphql`)
    #[arg(long)]
    graphql_dir: Option<PathBuf>,

    #[arg(long, default_value = "schema/schema.graphql")]
    out: PathBuf,
}

fn main() -> anyhow::Result<()> {
    let args = Args::parse();

    let graphql_dir = match args.graphql_dir {
        Some(p) => p,
        None => newest_graphql_dir()?,
    };

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

    let content = copilot_money_api::schema_gen::render_schema_from_operations(&docs)?;
    if let Some(parent) = args.out.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(&args.out, content)?;
    Ok(())
}

fn newest_graphql_dir() -> anyhow::Result<PathBuf> {
    let mut best: Option<PathBuf> = None;
    for entry in fs::read_dir("artifacts/graphql-ops")? {
        let entry = entry?;
        let p = entry.path().join("graphql");
        if !p.is_dir() {
            continue;
        }
        match &best {
            None => best = Some(p),
            Some(prev) => {
                if p > *prev {
                    best = Some(p);
                }
            }
        }
    }
    best.ok_or_else(|| anyhow::anyhow!("no graphql capture dirs found under artifacts/graphql-ops"))
}
