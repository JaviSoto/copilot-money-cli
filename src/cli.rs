use std::path::PathBuf;

use anyhow::Context;
use clap::{Args, Parser, Subcommand, ValueEnum};
use serde::Serialize;
use tabled::Table;
use tabled::settings::Style;

use crate::client::{ClientMode, CopilotClient};
use crate::config::{load_token, save_token, token_path};

#[derive(Debug, Clone, Copy, ValueEnum, Serialize)]
pub enum OutputFormat {
    Json,
    Table,
}

#[derive(Debug, Clone, Parser)]
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

    #[arg(long, global = true)]
    pub apply: bool,

    #[arg(long, global = true)]
    pub dry_run: bool,

    #[arg(
        long,
        global = true,
        env = "COPILOT_BASE_URL",
        default_value = "https://app.copilot.money"
    )]
    pub base_url: String,

    #[arg(long, global = true, env = "COPILOT_TOKEN")]
    pub token: Option<String>,

    #[arg(long, global = true, env = "COPILOT_TOKEN_FILE")]
    pub token_file: Option<PathBuf>,

    #[arg(long, global = true, env = "COPILOT_FIXTURES_DIR", hide = true)]
    pub fixtures_dir: Option<PathBuf>,
}

#[derive(Debug, Clone, Subcommand)]
pub enum Command {
    Auth {
        #[command(subcommand)]
        cmd: AuthCmd,
    },
    Transactions {
        #[command(subcommand)]
        cmd: TransactionsCmd,
    },
    Categories {
        #[command(subcommand)]
        cmd: CategoriesCmd,
    },
    Recurrings {
        #[command(subcommand)]
        cmd: RecurringsCmd,
    },
    Budgets {
        #[command(subcommand)]
        cmd: BudgetsCmd,
    },
    Undo,
    Version,
}

#[derive(Debug, Clone, Subcommand)]
pub enum AuthCmd {
    Status,
    Login(AuthLoginArgs),
    Logout,
}

#[derive(Debug, Clone, Args)]
pub struct AuthLoginArgs {
    #[arg(long, default_value = "tools/get_token.py")]
    pub helper: PathBuf,

    #[arg(long)]
    pub secrets_file: Option<PathBuf>,
}

#[derive(Debug, Clone, Subcommand)]
pub enum TransactionsCmd {
    List(TransactionsListArgs),
    Search(TransactionsSearchArgs),
    Show(TransactionsShowArgs),
    Review(TransactionsReviewArgs),
    Unreview(TransactionsReviewArgs),
    SetCategory(TransactionsSetCategoryArgs),
    AssignRecurring(TransactionsAssignRecurringArgs),
}

#[derive(Debug, Clone, Args)]
pub struct TransactionsListArgs {
    #[arg(long, default_value_t = 25)]
    pub limit: usize,
}

#[derive(Debug, Clone, Args)]
pub struct TransactionsSearchArgs {
    pub query: String,

    #[arg(long, default_value_t = 200)]
    pub limit: usize,
}

#[derive(Debug, Clone, Args)]
pub struct TransactionsShowArgs {
    pub id: String,

    #[arg(long, default_value_t = 200)]
    pub limit: usize,
}

#[derive(Debug, Clone, Args)]
pub struct TransactionsReviewArgs {
    pub ids: Vec<String>,
}

#[derive(Debug, Clone, Args)]
pub struct TransactionsSetCategoryArgs {
    pub ids: Vec<String>,
    pub category_id: String,
}

#[derive(Debug, Clone, Args)]
pub struct TransactionsAssignRecurringArgs {
    pub ids: Vec<String>,
    pub recurring_id: String,
}

#[derive(Debug, Clone, Subcommand)]
pub enum CategoriesCmd {
    List,
    Show { id: String },
}

#[derive(Debug, Clone, Subcommand)]
pub enum RecurringsCmd {
    List,
    Show { id: String },
    Create,
}

#[derive(Debug, Clone, Subcommand)]
pub enum BudgetsCmd {
    Month,
    Set,
}

#[derive(Debug, Clone, Serialize, tabled::Tabled)]
pub struct KeyValueRow {
    pub key: String,
    pub value: String,
}

pub fn run(cli: Cli) -> anyhow::Result<()> {
    if let Command::Version = &cli.command {
        println!("copilot-money-api");
        return Ok(());
    }

    let token = match cli.token.clone() {
        Some(t) => Some(t),
        None => {
            let p = cli.token_file.clone().unwrap_or_else(token_path);
            load_token(&p).ok()
        }
    };

    let mode = match &cli.fixtures_dir {
        Some(dir) => ClientMode::Fixtures(dir.clone()),
        None => ClientMode::Http {
            base_url: cli.base_url.clone(),
            token,
        },
    };
    let client = CopilotClient::new(mode);

    match &cli.command {
        Command::Auth { cmd } => run_auth(&cli, &client, cmd.clone()),
        Command::Transactions { cmd } => run_transactions(&cli, &client, cmd.clone()),
        Command::Categories { cmd } => run_categories(&cli, &client, cmd.clone()),
        Command::Recurrings { cmd } => run_recurrings(&cli, &client, cmd.clone()),
        Command::Budgets { cmd } => run_budgets(&cli, &client, cmd.clone()),
        Command::Undo => anyhow::bail!("undo not implemented yet"),
        Command::Version => unreachable!(),
    }
}

fn run_auth(cli: &Cli, client: &CopilotClient, cmd: AuthCmd) -> anyhow::Result<()> {
    match cmd {
        AuthCmd::Status => {
            let token = match cli.token.clone() {
                Some(t) => Some(("env".to_string(), t)),
                None => {
                    let p = cli.token_file.clone().unwrap_or_else(token_path);
                    load_token(&p).ok().map(|t| ("file".to_string(), t))
                }
            };

            let mut rows = Vec::new();
            rows.push(KeyValueRow {
                key: "token_configured".to_string(),
                value: token.is_some().to_string(),
            });

            let valid = token.as_ref().map(|_| client.try_user_query().is_ok());
            rows.push(KeyValueRow {
                key: "token_valid".to_string(),
                value: valid
                    .map(|v| v.to_string())
                    .unwrap_or_else(|| "unknown".to_string()),
            });

            render_output(cli.output, rows)
        }
        AuthCmd::Login(args) => {
            if !cli.apply {
                anyhow::bail!("refusing to write token without --apply");
            }
            if cli.dry_run {
                println!("dry-run: would obtain token via {}", args.helper.display());
                return Ok(());
            }

            let mut cmd = std::process::Command::new("python3");
            cmd.arg(args.helper);
            if let Some(p) = args.secrets_file {
                cmd.args(["--secrets-file", p.to_string_lossy().as_ref()]);
            }
            let out = cmd.output().context("failed to run token helper")?;
            if !out.status.success() {
                anyhow::bail!("token helper failed");
            }
            let token = String::from_utf8(out.stdout)?.trim().to_string();
            if token.is_empty() {
                anyhow::bail!("token helper returned empty token");
            }

            let p = cli.token_file.clone().unwrap_or_else(token_path);
            save_token(&p, &token)?;

            println!("saved token to {}", p.display());
            Ok(())
        }
        AuthCmd::Logout => {
            if !cli.apply {
                anyhow::bail!("refusing to delete token without --apply");
            }
            let p = cli.token_file.clone().unwrap_or_else(token_path);
            if p.exists() {
                std::fs::remove_file(&p)?;
            }
            println!("removed token at {}", p.display());
            Ok(())
        }
    }
}

#[derive(Debug, Clone, Serialize, tabled::Tabled)]
struct TransactionRow {
    id: String,
    date: String,
    name: String,
    amount: String,
    reviewed: String,
    category_id: String,
}

fn run_transactions(cli: &Cli, client: &CopilotClient, cmd: TransactionsCmd) -> anyhow::Result<()> {
    match cmd {
        TransactionsCmd::List(args) => {
            let items = client.list_transactions(args.limit)?;
            let rows = items
                .into_iter()
                .map(|t| TransactionRow {
                    id: t.id,
                    date: t.date.unwrap_or_default(),
                    name: t.name.unwrap_or_default(),
                    amount: t.amount.unwrap_or_default(),
                    reviewed: t.is_reviewed.unwrap_or(false).to_string(),
                    category_id: t.category_id.unwrap_or_default(),
                })
                .collect::<Vec<_>>();
            render_output(cli.output, rows)
        }
        TransactionsCmd::Search(args) => {
            let items = client.list_transactions(args.limit)?;
            let q = args.query.to_lowercase();
            let filtered = items
                .into_iter()
                .filter(|t| t.name.as_deref().unwrap_or("").to_lowercase().contains(&q))
                .collect::<Vec<_>>();

            let rows = filtered
                .into_iter()
                .map(|t| TransactionRow {
                    id: t.id,
                    date: t.date.unwrap_or_default(),
                    name: t.name.unwrap_or_default(),
                    amount: t.amount.unwrap_or_default(),
                    reviewed: t.is_reviewed.unwrap_or(false).to_string(),
                    category_id: t.category_id.unwrap_or_default(),
                })
                .collect::<Vec<_>>();
            render_output(cli.output, rows)
        }
        TransactionsCmd::Show(args) => {
            let items = client.list_transactions(args.limit)?;
            let found = items.into_iter().find(|t| t.id == args.id);
            match found {
                Some(t) => render_output(
                    cli.output,
                    vec![
                        KeyValueRow {
                            key: "id".to_string(),
                            value: t.id,
                        },
                        KeyValueRow {
                            key: "date".to_string(),
                            value: t.date.unwrap_or_default(),
                        },
                        KeyValueRow {
                            key: "name".to_string(),
                            value: t.name.unwrap_or_default(),
                        },
                        KeyValueRow {
                            key: "amount".to_string(),
                            value: t.amount.unwrap_or_default(),
                        },
                        KeyValueRow {
                            key: "category_id".to_string(),
                            value: t.category_id.unwrap_or_default(),
                        },
                        KeyValueRow {
                            key: "reviewed".to_string(),
                            value: t.is_reviewed.unwrap_or(false).to_string(),
                        },
                    ],
                ),
                None => anyhow::bail!("transaction not found"),
            }
        }
        TransactionsCmd::Review(args) => {
            if !cli.apply {
                println!("dry-run: would mark reviewed: {:?}", args.ids);
                return Ok(());
            }
            anyhow::bail!("review mutation not implemented yet (need captured mutation document)")
        }
        TransactionsCmd::Unreview(args) => {
            if !cli.apply {
                println!("dry-run: would mark unreviewed: {:?}", args.ids);
                return Ok(());
            }
            anyhow::bail!("unreview mutation not implemented yet (need captured mutation document)")
        }
        TransactionsCmd::SetCategory(args) => {
            if !cli.apply {
                println!(
                    "dry-run: would set category {} for {:?}",
                    args.category_id, args.ids
                );
                return Ok(());
            }
            anyhow::bail!(
                "set-category mutation not implemented yet (need captured mutation document)"
            )
        }
        TransactionsCmd::AssignRecurring(args) => {
            if !cli.apply {
                println!(
                    "dry-run: would assign recurring {} for {:?}",
                    args.recurring_id, args.ids
                );
                return Ok(());
            }
            anyhow::bail!(
                "assign-recurring mutation not implemented yet (need captured mutation document)"
            )
        }
    }
}

#[derive(Debug, Clone, Serialize, tabled::Tabled)]
struct CategoryRow {
    id: String,
    name: String,
}

fn run_categories(cli: &Cli, client: &CopilotClient, cmd: CategoriesCmd) -> anyhow::Result<()> {
    match cmd {
        CategoriesCmd::List => {
            let items = client.list_categories()?;
            let rows = items
                .into_iter()
                .map(|c| CategoryRow {
                    id: c.id,
                    name: c.name,
                })
                .collect::<Vec<_>>();
            render_output(cli.output, rows)
        }
        CategoriesCmd::Show { id } => {
            let items = client.list_categories()?;
            let found = items.into_iter().find(|c| c.id == id);
            match found {
                Some(c) => render_output(
                    cli.output,
                    vec![
                        KeyValueRow {
                            key: "id".to_string(),
                            value: c.id,
                        },
                        KeyValueRow {
                            key: "name".to_string(),
                            value: c.name,
                        },
                    ],
                ),
                None => anyhow::bail!("category not found"),
            }
        }
    }
}

#[derive(Debug, Clone, Serialize, tabled::Tabled)]
struct RecurringRow {
    id: String,
    name: String,
    frequency: String,
    category_id: String,
}

fn run_recurrings(cli: &Cli, client: &CopilotClient, cmd: RecurringsCmd) -> anyhow::Result<()> {
    match cmd {
        RecurringsCmd::List => {
            let items = client.list_recurrings()?;
            let rows = items
                .into_iter()
                .map(|r| RecurringRow {
                    id: r.id,
                    name: r.name.unwrap_or_default(),
                    frequency: r.frequency.unwrap_or_default(),
                    category_id: r.category_id.unwrap_or_default(),
                })
                .collect::<Vec<_>>();
            render_output(cli.output, rows)
        }
        RecurringsCmd::Show { id } => {
            let items = client.list_recurrings()?;
            let found = items.into_iter().find(|r| r.id == id);
            match found {
                Some(r) => render_output(
                    cli.output,
                    vec![
                        KeyValueRow {
                            key: "id".to_string(),
                            value: r.id,
                        },
                        KeyValueRow {
                            key: "name".to_string(),
                            value: r.name.unwrap_or_default(),
                        },
                        KeyValueRow {
                            key: "frequency".to_string(),
                            value: r.frequency.unwrap_or_default(),
                        },
                        KeyValueRow {
                            key: "category_id".to_string(),
                            value: r.category_id.unwrap_or_default(),
                        },
                    ],
                ),
                None => anyhow::bail!("recurring not found"),
            }
        }
        RecurringsCmd::Create => {
            anyhow::bail!("recurrings create not implemented yet (need mutation doc)")
        }
    }
}

#[derive(Debug, Clone, Serialize, tabled::Tabled)]
struct BudgetRow {
    month: String,
    amount: String,
}

fn run_budgets(cli: &Cli, client: &CopilotClient, cmd: BudgetsCmd) -> anyhow::Result<()> {
    match cmd {
        BudgetsCmd::Month => {
            let items = client.list_budget_months()?;
            let rows = items
                .into_iter()
                .map(|b| BudgetRow {
                    month: b.month,
                    amount: b.amount,
                })
                .collect::<Vec<_>>();
            render_output(cli.output, rows)
        }
        BudgetsCmd::Set => anyhow::bail!("budgets set not implemented yet (need mutation doc)"),
    }
}

fn render_output<T: Serialize + tabled::Tabled>(
    fmt: OutputFormat,
    rows: Vec<T>,
) -> anyhow::Result<()> {
    match fmt {
        OutputFormat::Json => {
            let s = serde_json::to_string_pretty(&rows)?;
            println!("{s}");
            Ok(())
        }
        OutputFormat::Table => {
            let mut table = Table::new(rows);
            table.with(Style::rounded());
            println!("{table}");
            Ok(())
        }
    }
}
