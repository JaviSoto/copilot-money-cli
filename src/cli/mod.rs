use std::collections::{HashMap, HashSet};
use std::io::IsTerminal;
use std::path::PathBuf;

use anyhow::Context;
use clap::builder::ArgGroup;
use clap::{Args, Parser, Subcommand, ValueEnum};
use comfy_table::modifiers::UTF8_ROUND_CORNERS;
use comfy_table::presets::UTF8_FULL;
use comfy_table::{Cell, Color, ContentArrangement, Row as ComfyRow, Table};
use serde::Serialize;

use crate::client::{
    BulkEditTransactionsResult, Category, ClientMode, CopilotClient, PageInfo, Transaction,
    TransactionIdRef,
};
use crate::config::{load_token, session_path, token_path};
use crate::types::{
    CategoryId, RecurringFrequency, RecurringId, TagId, TransactionId, TransactionType,
};

mod auth;
mod budgets;
mod categories;
mod recurrings;
mod render;
mod tags;
use render::{
    KeyValueRow, TableRow, header_cell, render_output, shorten_id_for_table, terminal_width,
};

#[derive(Debug, Clone, Copy, ValueEnum, Serialize, PartialEq, Eq)]
pub enum OutputFormat {
    Json,
    Table,
}

#[derive(Debug, Clone, Copy, ValueEnum, PartialEq, Eq)]
pub enum ColorMode {
    Auto,
    Always,
    Never,
}

#[derive(Debug, Clone, Parser)]
#[command(name = "copilot")]
#[command(about = "CLI for Copilot Money (unofficial)", version)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,

    #[arg(long, value_enum, default_value_t = OutputFormat::Table, global = true)]
    pub output: OutputFormat,

    #[arg(long, value_enum, default_value_t = ColorMode::Auto, global = true)]
    pub color: ColorMode,

    #[arg(long, global = true)]
    pub dry_run: bool,

    /// Skip confirmation prompts for write actions (required in non-interactive runs).
    #[arg(long, global = true, default_value_t = false)]
    pub yes: bool,

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

    #[arg(long, global = true, env = "COPILOT_SESSION_DIR")]
    pub session_dir: Option<PathBuf>,

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
    Tags {
        #[command(subcommand)]
        cmd: TagsCmd,
    },
    Budgets {
        #[command(subcommand)]
        cmd: BudgetsCmd,
    },
    Version,
}

#[derive(Debug, Clone, Subcommand)]
pub enum AuthCmd {
    Status,
    Login(AuthLoginArgs),
    Refresh(AuthRefreshArgs),
    SetToken(AuthSetTokenArgs),
    Logout,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum AuthLoginMode {
    /// Opens a browser and waits for you to log in.
    Interactive,
    /// Sends a magic link email; paste the link back (SSH-friendly).
    EmailLink,
    /// Uses `--secrets-file` with email+password (not recommended for open-source).
    Credentials,
}

#[derive(Debug, Clone, Args)]
pub struct AuthLoginArgs {
    #[arg(long)]
    pub secrets_file: Option<PathBuf>,

    #[arg(long, value_enum, default_value_t = AuthLoginMode::Interactive)]
    pub mode: AuthLoginMode,

    /// Required for `--mode email-link` unless it can be inferred from `--secrets-file`.
    #[arg(long)]
    pub email: Option<String>,

    #[arg(long, default_value_t = 180)]
    pub timeout_seconds: u64,

    /// Store a persistent browser session so tokens can be refreshed without re-auth.
    #[arg(long, default_value_t = false)]
    pub persist_session: bool,
}

#[derive(Debug, Clone, Args)]
pub struct AuthRefreshArgs {
    #[arg(long, default_value_t = 180)]
    pub timeout_seconds: u64,
}

#[derive(Debug, Clone, Args)]
pub struct AuthSetTokenArgs {
    /// Where to store the token (defaults to `~/.config/copilot-money-cli/token`)
    #[arg(long)]
    pub token_file: Option<PathBuf>,
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
    SetNotes(TransactionsSetNotesArgs),
    SetTags(TransactionsSetTagsArgs),
    Edit(TransactionsEditArgs),
}

#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum TransactionsSort {
    DateDesc,
    DateAsc,
    AmountDesc,
    AmountAsc,
}

fn sort_to_graphql(sort: Option<TransactionsSort>) -> Option<serde_json::Value> {
    let s = sort?;
    let (field, direction) = match s {
        TransactionsSort::DateDesc => ("DATE", "DESC"),
        TransactionsSort::DateAsc => ("DATE", "ASC"),
        TransactionsSort::AmountDesc => ("AMOUNT", "DESC"),
        TransactionsSort::AmountAsc => ("AMOUNT", "ASC"),
    };
    Some(serde_json::json!([{ "field": field, "direction": direction }]))
}

#[derive(Debug, Clone, Copy, ValueEnum, PartialEq, Eq)]
pub enum TransactionField {
    Date,
    Name,
    Amount,
    Reviewed,
    Category,
    Tags,
    Type,
    Id,
}

#[derive(Debug, Clone, Args)]
pub struct TransactionsListArgs {
    #[arg(long, default_value_t = 25)]
    pub limit: usize,

    /// Cursor to continue pagination from a previous call (`pageInfo.endCursor`).
    #[arg(long)]
    pub after: Option<String>,

    /// Number of pages to fetch (each page is `--limit`).
    #[arg(long, default_value_t = 1)]
    pub pages: usize,

    /// Fetch all pages until exhausted (can be slow).
    #[arg(long, default_value_t = false, conflicts_with = "pages")]
    pub all: bool,

    /// Filter to reviewed transactions only.
    #[arg(long, default_value_t = false, conflicts_with = "unreviewed")]
    pub reviewed: bool,

    /// Filter to unreviewed transactions only.
    #[arg(long, default_value_t = false, conflicts_with = "reviewed")]
    pub unreviewed: bool,

    /// Filter to a specific category id.
    #[arg(long)]
    pub category_id: Option<CategoryId>,

    /// Filter to a specific category by name (case-insensitive exact match).
    #[arg(long, conflicts_with = "category_id")]
    pub category: Option<String>,

    /// Filter to transactions that include any of these tags (repeatable).
    #[arg(long, value_name = "TAG")]
    pub tag: Vec<String>,

    /// Filter to a specific date (supports YYYY-MM-DD and MM-DD-YYYY).
    #[arg(long)]
    pub date: Option<String>,

    /// Filter by merchant/name substring (case-insensitive).
    #[arg(long)]
    pub name_contains: Option<String>,

    /// Sort transactions server-side (best-effort).
    #[arg(long, value_enum)]
    pub sort: Option<TransactionsSort>,

    /// Columns to show in table output (comma-separated).
    #[arg(
        long,
        value_enum,
        value_delimiter = ',',
        default_value = "date,name,amount,reviewed,category,tags,type"
    )]
    pub fields: Vec<TransactionField>,

    /// Include pagination info (`pageInfo`) in the output.
    #[arg(long, default_value_t = false)]
    pub page_info: bool,
}

#[derive(Debug, Clone, Args)]
pub struct TransactionsSearchArgs {
    pub query: String,

    #[arg(long, default_value_t = 200)]
    pub limit: usize,

    /// Cursor to continue pagination from a previous call (`pageInfo.endCursor`).
    #[arg(long)]
    pub after: Option<String>,

    /// Number of pages to fetch (each page is `--limit`).
    #[arg(long, default_value_t = 1)]
    pub pages: usize,

    /// Fetch all pages until exhausted (can be slow).
    #[arg(long, default_value_t = false, conflicts_with = "pages")]
    pub all: bool,

    /// Filter to reviewed transactions only.
    #[arg(long, default_value_t = false, conflicts_with = "unreviewed")]
    pub reviewed: bool,

    /// Filter to unreviewed transactions only.
    #[arg(long, default_value_t = false, conflicts_with = "reviewed")]
    pub unreviewed: bool,

    /// Filter to a specific category id.
    #[arg(long)]
    pub category_id: Option<CategoryId>,

    /// Filter to a specific category by name (case-insensitive exact match).
    #[arg(long, conflicts_with = "category_id")]
    pub category: Option<String>,

    /// Filter to transactions that include any of these tags (repeatable).
    #[arg(long, value_name = "TAG")]
    pub tag: Vec<String>,

    /// Filter to a specific date (supports YYYY-MM-DD and MM-DD-YYYY).
    #[arg(long)]
    pub date: Option<String>,

    /// Sort transactions server-side (best-effort).
    #[arg(long, value_enum)]
    pub sort: Option<TransactionsSort>,

    /// Columns to show in table output (comma-separated).
    #[arg(
        long,
        value_enum,
        value_delimiter = ',',
        default_value = "date,name,amount,reviewed,category,tags,type"
    )]
    pub fields: Vec<TransactionField>,

    /// Include pagination info (`pageInfo`) in the output.
    #[arg(long, default_value_t = false)]
    pub page_info: bool,
}

#[derive(Debug, Clone, Args)]
pub struct TransactionsShowArgs {
    pub id: TransactionId,

    #[arg(long, default_value_t = 200)]
    pub limit: usize,
}

#[derive(Debug, Clone, Args)]
pub struct TransactionsReviewArgs {
    pub ids: Vec<TransactionId>,
}

#[derive(Debug, Clone, Args)]
#[command(group(
    ArgGroup::new("category_target")
        .required(true)
        .args(["category_id", "category"])
))]
pub struct TransactionsSetCategoryArgs {
    pub ids: Vec<TransactionId>,

    #[arg(long)]
    pub category_id: Option<CategoryId>,

    #[arg(long)]
    pub category: Option<String>,
}

#[derive(Debug, Clone, Args)]
pub struct TransactionsAssignRecurringArgs {
    pub ids: Vec<TransactionId>,

    #[arg(long)]
    pub recurring_id: RecurringId,
}

#[derive(Debug, Clone, Args)]
pub struct TransactionsSetNotesArgs {
    pub ids: Vec<TransactionId>,

    #[arg(long, conflicts_with = "clear")]
    pub notes: Option<String>,

    #[arg(long, default_value_t = false)]
    pub clear: bool,
}

#[derive(Debug, Clone, Copy, ValueEnum, PartialEq, Eq)]
pub enum TagUpdateMode {
    Set,
    Add,
    Remove,
}

#[derive(Debug, Clone, Args)]
pub struct TransactionsSetTagsArgs {
    pub ids: Vec<TransactionId>,

    #[arg(long, value_enum, default_value_t = TagUpdateMode::Set)]
    pub mode: TagUpdateMode,

    /// One or more tag IDs (repeatable).
    #[arg(long = "tag-id", value_name = "TAG_ID")]
    pub tag_ids: Vec<crate::types::TagId>,
}

#[derive(Debug, Clone, Args)]
#[command(group(
    ArgGroup::new("edit_input")
        .required(true)
        .args(["type_", "input_json"])
))]
pub struct TransactionsEditArgs {
    pub ids: Vec<TransactionId>,

    /// Set transaction type (best-effort; server enum values vary).
    #[arg(long = "type")]
    pub type_: Option<TransactionType>,

    /// Raw JSON to pass as EditTransactionInput (advanced).
    #[arg(long)]
    pub input_json: Option<String>,
}

#[derive(Debug, Clone, Subcommand)]
pub enum CategoriesCmd {
    List(CategoriesListArgs),
    Show { id: CategoryId },
    Create(CategoriesCreateArgs),
    Edit(CategoriesEditArgs),
}

#[derive(Debug, Clone, Args)]
pub struct CategoriesListArgs {
    /// Include spend data (current + history).
    #[arg(long, default_value_t = false)]
    pub spend: bool,

    /// Include budget data (current + history).
    #[arg(long, default_value_t = false)]
    pub budget: bool,

    /// When used with `--budget`, request rollover-enabled budgets.
    #[arg(long, default_value_t = false)]
    pub rollovers: bool,

    /// Include child categories (nested categories).
    #[arg(long, default_value_t = false)]
    pub children: bool,

    /// Filter by name substring (case-insensitive).
    #[arg(long)]
    pub name_contains: Option<String>,
}

#[derive(Debug, Clone, Args)]
pub struct CategoriesCreateArgs {
    pub name: String,

    #[arg(long)]
    pub emoji: Option<String>,

    #[arg(long)]
    pub color_name: Option<String>,

    #[arg(long, default_value_t = false)]
    pub excluded: bool,

    #[arg(long)]
    pub template_id: Option<String>,

    /// When set, include an initial budget in the category input.
    #[arg(long)]
    pub budget_unassigned_amount: Option<i64>,
}

#[derive(Debug, Clone, Args)]
pub struct CategoriesEditArgs {
    pub id: String,

    #[arg(long)]
    pub name: Option<String>,

    #[arg(long)]
    pub emoji: Option<String>,

    #[arg(long)]
    pub color_name: Option<String>,

    #[arg(long)]
    pub excluded: Option<bool>,
}

#[derive(Debug, Clone, Subcommand)]
pub enum RecurringsCmd {
    List(RecurringsListArgs),
    Show { id: RecurringId },
    Create(RecurringsCreateArgs),
    Edit(RecurringsEditArgs),
}

#[derive(Debug, Clone, Subcommand)]
pub enum TagsCmd {
    List,
    Create(TagsCreateArgs),
    Delete(TagsDeleteArgs),
}

#[derive(Debug, Clone, Args)]
pub struct TagsCreateArgs {
    pub name: String,

    #[arg(long)]
    pub color_name: Option<String>,
}

#[derive(Debug, Clone, Args)]
pub struct TagsDeleteArgs {
    pub id: crate::types::TagId,
}

#[derive(Debug, Clone, Args)]
pub struct RecurringsListArgs {
    /// Filter to a specific category id.
    #[arg(long)]
    pub category_id: Option<CategoryId>,

    /// Filter by name substring (case-insensitive).
    #[arg(long)]
    pub name_contains: Option<String>,
}

#[derive(Debug, Clone, Args)]
pub struct RecurringsCreateArgs {
    /// A transaction ID to derive the recurring rule from.
    pub transaction_id: TransactionId,

    /// Recurring frequency (best-effort; Copilot expects values like ANNUALLY, MONTHLY, etc).
    #[arg(long)]
    pub frequency: RecurringFrequency,
}

#[derive(Debug, Clone, Args)]
pub struct RecurringsEditArgs {
    pub id: RecurringId,

    #[arg(long)]
    pub name_contains: Option<String>,

    #[arg(long)]
    pub min_amount: Option<i64>,

    #[arg(long)]
    pub max_amount: Option<i64>,

    #[arg(long, default_value_t = false)]
    pub recalculate_only_for_future: bool,
}

#[derive(Debug, Clone, Subcommand)]
pub enum BudgetsCmd {
    Month,
    Set,
}

pub fn run(cli: Cli) -> anyhow::Result<()> {
    if let Command::Version = &cli.command {
        println!("copilot-money-cli {}", env!("CARGO_PKG_VERSION"));
        return Ok(());
    }

    let token_file_path = cli.token_file.clone().unwrap_or_else(token_path);
    let token = cli
        .token
        .clone()
        .or_else(|| load_token(&token_file_path).ok());

    let mode = match &cli.fixtures_dir {
        Some(dir) => ClientMode::Fixtures(dir.clone()),
        None => ClientMode::Http {
            base_url: cli.base_url.clone(),
            token,
            token_file: token_file_path.clone(),
            session_dir: cli
                .session_dir
                .clone()
                .or_else(|| session_path().exists().then_some(session_path())),
        },
    };
    let client = CopilotClient::new(mode);

    match &cli.command {
        Command::Auth { cmd } => auth::run_auth(&cli, &client, cmd.clone()),
        Command::Transactions { cmd } => run_transactions(&cli, &client, cmd.clone()),
        Command::Categories { cmd } => categories::run_categories(&cli, &client, cmd.clone()),
        Command::Recurrings { cmd } => recurrings::run_recurrings(&cli, &client, cmd.clone()),
        Command::Tags { cmd } => tags::run_tags(&cli, &client, cmd.clone()),
        Command::Budgets { cmd } => budgets::run_budgets(&cli, &client, cmd.clone()),
        Command::Version => unreachable!(),
    }
}

impl TableRow for KeyValueRow {
    const HEADERS: &'static [&'static str] = &["key", "value"];

    fn cells(&self) -> Vec<Cell> {
        vec![Cell::new(&self.key), Cell::new(&self.value)]
    }
}

fn value_to_string(v: Option<serde_json::Value>) -> String {
    match v {
        None => String::new(),
        Some(serde_json::Value::String(s)) => s,
        Some(serde_json::Value::Number(n)) => n.to_string(),
        Some(serde_json::Value::Bool(b)) => b.to_string(),
        Some(serde_json::Value::Null) => String::new(),
        Some(other) => other.to_string(),
    }
}

fn value_to_money_string(v: Option<serde_json::Value>) -> String {
    let s = value_to_string(v);
    if s.trim().is_empty() {
        return String::new();
    }

    // Common cases from Copilot: "-57.48" or 185.4 (already stringified).
    let trimmed = s.trim();
    let negative = trimmed.starts_with('-');
    let numeric = trimmed.trim_start_matches('-');

    if let Ok(n) = numeric.parse::<f64>() {
        let formatted = format!("{:.2}", n.abs());
        if negative {
            format!("-${formatted}")
        } else {
            format!("${formatted}")
        }
    } else {
        // Fallback: keep original, but prefix `$` if it looks like a number.
        if negative {
            format!("-${numeric}")
        } else {
            format!("${trimmed}")
        }
    }
}

fn normalize_date(s: &str) -> Option<String> {
    let s = s.trim();
    if s.len() != 10 {
        return None;
    }

    let parts = s.split('-').collect::<Vec<_>>();
    if parts.len() != 3 {
        return None;
    }

    let (year, month, day) = if parts[0].len() == 4 {
        (parts[0], parts[1], parts[2])
    } else if parts[2].len() == 4 {
        (parts[2], parts[0], parts[1])
    } else {
        return None;
    };

    let y = year.parse::<u32>().ok()?;
    let m = month.parse::<u32>().ok()?;
    let d = day.parse::<u32>().ok()?;
    if !(1900..=2100).contains(&y) || !(1..=12).contains(&m) || !(1..=31).contains(&d) {
        return None;
    }
    Some(format!("{y:04}-{m:02}-{d:02}"))
}

fn build_transactions_filter(reviewed: bool, unreviewed: bool) -> Option<serde_json::Value> {
    if reviewed {
        Some(serde_json::json!({ "isReviewed": true }))
    } else if unreviewed {
        Some(serde_json::json!({ "isReviewed": false }))
    } else {
        None
    }
}

fn flatten_categories_for_lookup(categories: &[Category]) -> Vec<(CategoryId, String)> {
    fn walk(out: &mut Vec<(CategoryId, String)>, cats: &[Category]) {
        for c in cats {
            out.push((c.id.clone(), c.name.clone().unwrap_or_default()));
            if let Some(children) = c.child_categories.as_ref() {
                walk(out, children);
            }
        }
    }

    let mut out = Vec::new();
    walk(&mut out, categories);
    out
}

fn category_name_map(client: &CopilotClient) -> anyhow::Result<HashMap<CategoryId, String>> {
    let categories = client.list_categories(false, false, false)?;
    let mut out = HashMap::new();
    for (id, name) in flatten_categories_for_lookup(&categories) {
        out.insert(id, name);
    }
    Ok(out)
}

fn resolve_category_id(
    client: &CopilotClient,
    category_id: Option<&CategoryId>,
    category_name: Option<&str>,
) -> anyhow::Result<Option<CategoryId>> {
    if let Some(id) = category_id {
        return Ok(Some(id.clone()));
    }
    let Some(name) = category_name else {
        return Ok(None);
    };

    let want = name.trim().to_lowercase();
    if want.is_empty() {
        anyhow::bail!("empty --category");
    }

    let categories = client.list_categories(false, false, false)?;
    let matches = flatten_categories_for_lookup(&categories)
        .into_iter()
        .filter(|(_, n)| n.to_lowercase() == want)
        .collect::<Vec<_>>();

    match matches.as_slice() {
        [] => anyhow::bail!("no category named {:?}", name),
        [(id, _)] => Ok(Some(id.clone())),
        many => anyhow::bail!(
            "category name {:?} is ambiguous ({} matches); use --category-id instead",
            name,
            many.len()
        ),
    }
}

fn should_color(cli: &Cli) -> bool {
    match cli.color {
        ColorMode::Always => true,
        ColorMode::Never => false,
        ColorMode::Auto => std::io::stdout().is_terminal(),
    }
}

fn confirm_write(cli: &Cli, action: &str) -> anyhow::Result<()> {
    if cli.dry_run {
        return Ok(());
    }
    if cli.yes {
        return Ok(());
    }
    if !std::io::stdin().is_terminal() {
        anyhow::bail!("refusing to write in non-interactive mode without --yes");
    }

    eprintln!("{action}");
    let input = rpassword::prompt_password("Proceed? Type 'yes' to confirm: ")?;
    if input.trim() != "yes" {
        anyhow::bail!("aborted");
    }
    Ok(())
}

fn run_transactions(cli: &Cli, client: &CopilotClient, cmd: TransactionsCmd) -> anyhow::Result<()> {
    match cmd {
        TransactionsCmd::List(args) => {
            let category_id =
                resolve_category_id(client, args.category_id.as_ref(), args.category.as_deref())?;
            let filter = build_transactions_filter(args.reviewed, args.unreviewed);
            let sort = sort_to_graphql(args.sort);
            let (items, page_info) = fetch_transactions_with_filter_sort(
                client,
                args.limit,
                args.after.clone(),
                args.pages,
                args.all,
                filter,
                sort,
            )?;
            let filtered = filter_transactions(
                items,
                args.reviewed,
                args.unreviewed,
                category_id.as_ref(),
                &args.tag,
                args.name_contains.as_deref(),
                args.date.as_deref(),
            );
            render_transactions_output(
                cli,
                client,
                filtered,
                page_info,
                args.page_info,
                &args.fields,
            )
        }
        TransactionsCmd::Search(args) => {
            let category_id =
                resolve_category_id(client, args.category_id.as_ref(), args.category.as_deref())?;
            let filter = build_transactions_filter(args.reviewed, args.unreviewed);
            let sort = sort_to_graphql(args.sort);
            let (items, page_info) = fetch_transactions_with_filter_sort(
                client,
                args.limit,
                args.after.clone(),
                args.pages,
                args.all,
                filter,
                sort,
            )?;
            let filtered = filter_transactions(
                items,
                args.reviewed,
                args.unreviewed,
                category_id.as_ref(),
                &args.tag,
                Some(&args.query),
                args.date.as_deref(),
            );
            render_transactions_output(
                cli,
                client,
                filtered,
                page_info,
                args.page_info,
                &args.fields,
            )
        }
        TransactionsCmd::Show(args) => {
            let items = client.list_transactions(args.limit)?;
            let found = items.into_iter().find(|t| t.id == args.id);
            match found {
                Some(t) => render_output(
                    cli,
                    vec![
                        KeyValueRow {
                            key: "id".to_string(),
                            value: t.id.to_string(),
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
                            value: value_to_money_string(t.amount),
                        },
                        KeyValueRow {
                            key: "category_id".to_string(),
                            value: t
                                .category_id
                                .as_ref()
                                .map(|c| c.to_string())
                                .unwrap_or_default(),
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
            if cli.dry_run {
                println!("dry-run: would mark reviewed: {:?}", args.ids);
                return Ok(());
            }
            confirm_write(cli, &format!("Mark reviewed: {:?}", args.ids))?;
            let txns = resolve_transactions_by_ids(client, &args.ids)?;
            let refs = build_transaction_id_refs(&txns)?;
            let result = client.bulk_edit_transactions_reviewed(refs, true)?;
            render_bulk_edit_result(cli, result)
        }
        TransactionsCmd::Unreview(args) => {
            if cli.dry_run {
                println!("dry-run: would mark unreviewed: {:?}", args.ids);
                return Ok(());
            }
            confirm_write(cli, &format!("Mark unreviewed: {:?}", args.ids))?;
            let txns = resolve_transactions_by_ids(client, &args.ids)?;
            let refs = build_transaction_id_refs(&txns)?;
            let result = client.bulk_edit_transactions_reviewed(refs, false)?;
            render_bulk_edit_result(cli, result)
        }
        TransactionsCmd::SetCategory(args) => {
            if cli.dry_run {
                println!(
                    "dry-run: would set category {:?}/{:?} for {:?}",
                    args.category_id, args.category, args.ids
                );
                return Ok(());
            }
            let category_id =
                resolve_category_id(client, args.category_id.as_ref(), args.category.as_deref())?
                    .ok_or_else(|| anyhow::anyhow!("missing category target"))?;
            confirm_write(
                cli,
                &format!(
                    "Set category {:?}/{:?} for {:?}",
                    category_id, args.category, args.ids
                ),
            )?;
            let txns = resolve_transactions_by_ids(client, &args.ids)?;
            let mut updated = Vec::new();
            for txn in txns {
                let (item_id, account_id) = require_item_and_account(&txn)?;
                let t = client.edit_transaction(
                    &item_id,
                    &account_id,
                    &txn.id,
                    serde_json::json!({ "categoryId": category_id.clone() }),
                )?;
                updated.push(t);
            }
            render_transactions_updated(cli, updated)
        }
        TransactionsCmd::AssignRecurring(args) => {
            if cli.dry_run {
                println!(
                    "dry-run: would assign recurring {} for {:?}",
                    args.recurring_id, args.ids
                );
                return Ok(());
            }
            confirm_write(
                cli,
                &format!("Assign recurring {} for {:?}", args.recurring_id, args.ids),
            )?;
            let txns = resolve_transactions_by_ids(client, &args.ids)?;
            let mut updated = Vec::new();
            for txn in txns {
                let (item_id, account_id) = require_item_and_account(&txn)?;
                let t = client.add_transaction_to_recurring(
                    &item_id,
                    &account_id,
                    &txn.id,
                    &args.recurring_id,
                )?;
                updated.push(t);
            }
            render_transactions_updated(cli, updated)
        }
        TransactionsCmd::SetNotes(args) => {
            if cli.dry_run {
                println!(
                    "dry-run: would set notes for {:?} (clear={})",
                    args.ids, args.clear
                );
                return Ok(());
            }
            confirm_write(
                cli,
                &format!("Set notes for {:?} (clear={})", args.ids, args.clear),
            )?;
            if !args.clear && args.notes.is_none() {
                anyhow::bail!("use --notes <TEXT> or --clear");
            }
            let txns = resolve_transactions_by_ids(client, &args.ids)?;
            let mut updated = Vec::new();
            for txn in txns {
                let (item_id, account_id) = require_item_and_account(&txn)?;
                let input = if args.clear {
                    serde_json::json!({ "userNotes": "" })
                } else {
                    serde_json::json!({ "userNotes": args.notes.clone().unwrap_or_default() })
                };
                let t = client.edit_transaction(&item_id, &account_id, &txn.id, input)?;
                updated.push(t);
            }
            render_transactions_updated(cli, updated)
        }
        TransactionsCmd::SetTags(args) => {
            if cli.dry_run {
                println!(
                    "dry-run: would update tags mode={:?} tag_ids={:?} for {:?}",
                    args.mode, args.tag_ids, args.ids
                );
                return Ok(());
            }
            confirm_write(
                cli,
                &format!(
                    "Update tags mode={:?} tag_ids={:?} for {:?}",
                    args.mode, args.tag_ids, args.ids
                ),
            )?;
            if (args.mode == TagUpdateMode::Add || args.mode == TagUpdateMode::Remove)
                && args.tag_ids.is_empty()
            {
                anyhow::bail!("--tag-id is required for --mode add/remove");
            }

            let txns = resolve_transactions_by_ids(client, &args.ids)?;
            let mut updated = Vec::new();

            for txn in txns {
                let (item_id, account_id) = require_item_and_account(&txn)?;
                let existing = txn
                    .tags
                    .as_ref()
                    .map(|ts| ts.iter().map(|t| t.id.clone()).collect::<HashSet<_>>())
                    .unwrap_or_default();

                let next_ids: Vec<TagId> = match args.mode {
                    TagUpdateMode::Set => args.tag_ids.clone(),
                    TagUpdateMode::Add => {
                        let mut out = existing;
                        for id in &args.tag_ids {
                            out.insert(id.clone());
                        }
                        out.into_iter().collect()
                    }
                    TagUpdateMode::Remove => {
                        let mut out = existing;
                        for id in &args.tag_ids {
                            out.remove(id);
                        }
                        out.into_iter().collect()
                    }
                };

                let t = client.edit_transaction(
                    &item_id,
                    &account_id,
                    &txn.id,
                    serde_json::json!({
                        "tagIds": next_ids
                            .into_iter()
                            .map(|id| id.to_string())
                            .collect::<Vec<_>>()
                    }),
                )?;
                updated.push(t);
            }

            render_transactions_updated(cli, updated)
        }
        TransactionsCmd::Edit(args) => {
            if cli.dry_run {
                println!(
                    "dry-run: would edit transactions {:?} (type={:?}, input_json={})",
                    args.ids,
                    args.type_,
                    args.input_json.is_some()
                );
                return Ok(());
            }
            confirm_write(cli, &format!("Edit transactions {:?}", args.ids))?;

            let mut input = match args.input_json.as_ref() {
                None => serde_json::Value::Object(serde_json::Map::new()),
                Some(s) => serde_json::from_str::<serde_json::Value>(s)
                    .context("failed to parse --input-json")?,
            };

            if !input.is_object() {
                anyhow::bail!("--input-json must be a JSON object");
            }

            if let Some(t) = args.type_.as_ref() {
                input
                    .as_object_mut()
                    .expect("checked is_object above")
                    .insert("type".to_string(), serde_json::Value::String(t.to_string()));
            }

            let txns = resolve_transactions_by_ids(client, &args.ids)?;
            let mut updated = Vec::new();
            for txn in txns {
                let (item_id, account_id) = require_item_and_account(&txn)?;
                let t = client.edit_transaction(&item_id, &account_id, &txn.id, input.clone())?;
                updated.push(t);
            }
            render_transactions_updated(cli, updated)
        }
    }
}

fn require_item_and_account(
    txn: &Transaction,
) -> anyhow::Result<(crate::types::ItemId, crate::types::AccountId)> {
    let item_id = txn
        .item_id
        .clone()
        .ok_or_else(|| anyhow::anyhow!("transaction {} missing itemId", txn.id))?;
    let account_id = txn
        .account_id
        .clone()
        .ok_or_else(|| anyhow::anyhow!("transaction {} missing accountId", txn.id))?;
    Ok((item_id, account_id))
}

fn build_transaction_id_refs(txns: &[Transaction]) -> anyhow::Result<Vec<TransactionIdRef>> {
    let mut out = Vec::new();
    for txn in txns {
        let (item_id, account_id) = require_item_and_account(txn)?;
        out.push(TransactionIdRef {
            account_id,
            id: txn.id.clone(),
            item_id,
        });
    }
    Ok(out)
}

fn resolve_transactions_by_ids(
    client: &CopilotClient,
    ids: &[TransactionId],
) -> anyhow::Result<Vec<Transaction>> {
    let want: HashSet<TransactionId> = ids.iter().cloned().collect();
    let mut found: HashMap<TransactionId, Transaction> = HashMap::new();

    let mut cursor: Option<String> = None;
    let mut scanned = 0usize;
    let max_pages = 200usize; // safety guard; use `transactions list --all` if you need more context.

    for _ in 0..max_pages {
        let page = client.list_transactions_page(200, cursor.clone(), None, None)?;
        let has_next = page.page_info.has_next_page.unwrap_or(false);
        cursor = page.page_info.end_cursor.clone();
        scanned += page.transactions.len();

        for t in page.transactions {
            if want.contains(&t.id) {
                found.insert(t.id.clone(), t);
            }
        }

        if found.len() == want.len() {
            break;
        }

        if has_next {
            continue;
        }
        break;
    }

    let mut missing = Vec::new();
    let mut ordered = Vec::new();
    for id in ids {
        match found.remove(id) {
            Some(t) => ordered.push(t),
            None => missing.push(id.to_string()),
        }
    }

    if !missing.is_empty() {
        anyhow::bail!(
            "could not resolve {} transaction ids after scanning {scanned} transactions: {:?}",
            missing.len(),
            missing
        );
    }

    Ok(ordered)
}

#[derive(Debug, Serialize)]
struct BulkEditJsonOutput {
    updated: Vec<Transaction>,
    failed: Vec<crate::client::BulkEditFailed>,
}

fn render_bulk_edit_result(cli: &Cli, result: BulkEditTransactionsResult) -> anyhow::Result<()> {
    if !result.failed.is_empty() {
        if cli.output == OutputFormat::Json {
            let out = BulkEditJsonOutput {
                updated: result.updated,
                failed: result.failed,
            };
            let s = serde_json::to_string_pretty(&out)?;
            println!("{s}");
            return Ok(());
        }
        anyhow::bail!(
            "bulk edit failed for {} transaction(s)",
            result.failed.len()
        );
    }
    render_transactions_updated(cli, result.updated)
}

fn render_transactions_updated(cli: &Cli, items: Vec<Transaction>) -> anyhow::Result<()> {
    const DEFAULT_FIELDS: &[TransactionField] = &[
        TransactionField::Date,
        TransactionField::Name,
        TransactionField::Amount,
        TransactionField::Reviewed,
        TransactionField::Category,
        TransactionField::Tags,
        TransactionField::Type,
    ];

    match cli.output {
        OutputFormat::Json => {
            let out = TransactionsJsonOutput {
                transactions: items,
                page_info: None,
            };
            let s = serde_json::to_string_pretty(&out)?;
            println!("{s}");
            Ok(())
        }
        OutputFormat::Table => render_transactions_table(cli, &items, DEFAULT_FIELDS, None),
    }
}

#[derive(Debug, Serialize)]
struct TransactionsJsonOutput {
    transactions: Vec<Transaction>,
    #[serde(skip_serializing_if = "Option::is_none")]
    page_info: Option<PageInfo>,
}

fn fetch_transactions_with_filter_sort(
    client: &CopilotClient,
    page_size: usize,
    after: Option<String>,
    pages: usize,
    all: bool,
    filter: Option<serde_json::Value>,
    sort: Option<serde_json::Value>,
) -> anyhow::Result<(Vec<Transaction>, PageInfo)> {
    let mut out = Vec::new();
    let mut cursor = after;
    let max_pages = if all { usize::MAX } else { pages.max(1) };

    let mut last_page_info: Option<PageInfo> = None;

    for _ in 0..max_pages {
        let page = client.list_transactions_page(
            page_size,
            cursor.clone(),
            filter.clone(),
            sort.clone(),
        )?;
        cursor = page.page_info.end_cursor.clone();
        last_page_info = Some(page.page_info);
        out.extend(page.transactions);

        let has_next = last_page_info
            .as_ref()
            .and_then(|p| p.has_next_page)
            .unwrap_or(false);
        if !has_next || cursor.is_none() {
            break;
        }
    }

    Ok((
        out,
        last_page_info.unwrap_or(PageInfo {
            end_cursor: None,
            has_next_page: None,
            has_previous_page: None,
            start_cursor: None,
        }),
    ))
}

fn filter_transactions(
    items: Vec<Transaction>,
    reviewed: bool,
    unreviewed: bool,
    category_id: Option<&CategoryId>,
    tags: &[String],
    query: Option<&str>,
    date: Option<&str>,
) -> Vec<Transaction> {
    let q = query.map(|s| s.to_lowercase());
    let want_tags = tags.iter().map(|t| t.to_lowercase()).collect::<Vec<_>>();

    items
        .into_iter()
        .filter(|t| {
            if reviewed && !t.is_reviewed.unwrap_or(false) {
                return false;
            }
            if unreviewed && t.is_reviewed.unwrap_or(false) {
                return false;
            }
            if let Some(cat) = category_id
                && t.category_id.as_ref() != Some(cat)
            {
                return false;
            }
            if let Some(q) = &q {
                let name = t.name.as_deref().unwrap_or("").to_lowercase();
                if !name.contains(q) {
                    return false;
                }
            }
            if let Some(d) = date {
                let want = normalize_date(d).unwrap_or_else(|| d.to_string());
                if t.date.as_deref().unwrap_or("") != want {
                    return false;
                }
            }
            if want_tags.is_empty() {
                return true;
            }
            let txn_tags = t
                .tags
                .as_ref()
                .map(|ts| {
                    ts.iter()
                        .filter_map(|tag| tag.name.as_ref())
                        .map(|s| s.to_lowercase())
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default();
            txn_tags.iter().any(|t| want_tags.iter().any(|w| w == t))
        })
        .collect()
}

fn render_transactions_table(
    cli: &Cli,
    items: &[Transaction],
    fields: &[TransactionField],
    categories: Option<&HashMap<CategoryId, String>>,
) -> anyhow::Result<()> {
    use comfy_table::CellAlignment;

    let mut table = Table::new();
    table
        .load_preset(UTF8_FULL)
        .apply_modifier(UTF8_ROUND_CORNERS)
        .set_content_arrangement(ContentArrangement::DynamicFullWidth);

    if let Some(w) = terminal_width() {
        table.set_width(w);
    }

    let header = fields
        .iter()
        .map(|f| match f {
            TransactionField::Date => header_cell(cli, "date"),
            TransactionField::Name => header_cell(cli, "name"),
            TransactionField::Amount => header_cell(cli, "amount"),
            TransactionField::Reviewed => header_cell(cli, "reviewed"),
            TransactionField::Category => header_cell(cli, "category"),
            TransactionField::Tags => header_cell(cli, "tags"),
            TransactionField::Type => header_cell(cli, "type"),
            TransactionField::Id => header_cell(cli, "id"),
        })
        .collect::<Vec<_>>();
    table.set_header(ComfyRow::from(header));

    let use_color = should_color(cli);

    for t in items {
        let mut cells = Vec::new();
        for f in fields {
            match f {
                TransactionField::Date => cells.push(Cell::new(t.date.as_deref().unwrap_or(""))),
                TransactionField::Name => cells.push(Cell::new(t.name.as_deref().unwrap_or(""))),
                TransactionField::Amount => {
                    let s = value_to_money_string(t.amount.clone());
                    let mut cell = Cell::new(&s).set_alignment(CellAlignment::Right);
                    if use_color && !s.is_empty() {
                        if s.starts_with("-$") {
                            cell = cell.fg(Color::Red);
                        } else {
                            cell = cell.fg(Color::Green);
                        }
                    }
                    cells.push(cell);
                }
                TransactionField::Reviewed => {
                    let reviewed = t.is_reviewed.unwrap_or(false);
                    let mut cell = Cell::new(if reviewed { "✓" } else { "" });
                    if use_color && reviewed {
                        cell = cell.fg(Color::Green);
                    }
                    cells.push(cell);
                }
                TransactionField::Category => {
                    let name = t
                        .category_id
                        .as_ref()
                        .and_then(|id| categories.and_then(|m| m.get(id)))
                        .map(|s| s.as_str())
                        .or_else(|| t.category_id.as_ref().map(|id| id.as_str()))
                        .unwrap_or("");
                    cells.push(Cell::new(name));
                }
                TransactionField::Tags => {
                    let tags = t
                        .tags
                        .as_ref()
                        .map(|ts| {
                            ts.iter()
                                .filter_map(|tag| tag.name.as_deref())
                                .collect::<Vec<_>>()
                                .join(",")
                        })
                        .unwrap_or_default();
                    cells.push(Cell::new(tags));
                }
                TransactionField::Type => cells.push(Cell::new(
                    t.txn_type
                        .as_ref()
                        .map(|t| t.to_string())
                        .unwrap_or_default(),
                )),
                TransactionField::Id => cells.push(Cell::new(shorten_id_for_table(t.id.as_str()))),
            }
        }
        table.add_row(ComfyRow::from(cells));
    }

    println!("{table}");
    Ok(())
}

fn render_transactions_output(
    cli: &Cli,
    client: &CopilotClient,
    items: Vec<Transaction>,
    page_info: PageInfo,
    include_page_info: bool,
    fields: &[TransactionField],
) -> anyhow::Result<()> {
    match cli.output {
        OutputFormat::Json => {
            let out = TransactionsJsonOutput {
                transactions: items,
                page_info: include_page_info.then_some(page_info),
            };
            let s = serde_json::to_string_pretty(&out)?;
            println!("{s}");
            Ok(())
        }
        OutputFormat::Table => {
            let cats = if fields.contains(&TransactionField::Category) {
                Some(category_name_map(client)?)
            } else {
                None
            };
            render_transactions_table(cli, &items, fields, cats.as_ref())?;
            if include_page_info {
                render_output(
                    cli,
                    vec![
                        KeyValueRow {
                            key: "endCursor".to_string(),
                            value: page_info.end_cursor.unwrap_or_default(),
                        },
                        KeyValueRow {
                            key: "hasNextPage".to_string(),
                            value: page_info
                                .has_next_page
                                .map(|b| b.to_string())
                                .unwrap_or_default(),
                        },
                    ],
                )?;
            }
            Ok(())
        }
    }
}

#[cfg(test)]
mod helper_tests {
    use super::*;

    #[test]
    fn normalize_date_accepts_yyyy_mm_dd_and_mm_dd_yyyy() {
        assert_eq!(normalize_date("2025-12-03"), Some("2025-12-03".to_string()));
        assert_eq!(normalize_date("12-03-2025"), Some("2025-12-03".to_string()));
        assert_eq!(normalize_date("03-12-2025"), Some("2025-03-12".to_string()));
    }

    #[test]
    fn normalize_date_rejects_invalid() {
        assert_eq!(normalize_date(""), None);
        assert_eq!(normalize_date("2025-13-01"), None);
        assert_eq!(normalize_date("2025-00-01"), None);
        assert_eq!(normalize_date("2025-12-32"), None);
        assert_eq!(normalize_date("2025/12/01"), None);
    }

    #[test]
    fn money_string_formats_numbers() {
        assert_eq!(
            value_to_money_string(Some(serde_json::json!("-57.48"))),
            "-$57.48"
        );
        assert_eq!(
            value_to_money_string(Some(serde_json::json!(185.4))),
            "$185.40"
        );
        assert_eq!(value_to_money_string(Some(serde_json::json!("0"))), "$0.00");
        assert_eq!(value_to_money_string(None), "");
    }

    #[test]
    fn sort_to_graphql_maps_values() {
        assert_eq!(
            sort_to_graphql(Some(TransactionsSort::DateDesc)).unwrap(),
            serde_json::json!([{ "field": "DATE", "direction": "DESC" }])
        );
        assert_eq!(
            sort_to_graphql(Some(TransactionsSort::AmountAsc)).unwrap(),
            serde_json::json!([{ "field": "AMOUNT", "direction": "ASC" }])
        );
        assert!(sort_to_graphql(None).is_none());
    }

    #[test]
    fn build_transactions_filter_works() {
        assert_eq!(
            build_transactions_filter(true, false),
            Some(serde_json::json!({"isReviewed": true}))
        );
        assert_eq!(
            build_transactions_filter(false, true),
            Some(serde_json::json!({"isReviewed": false}))
        );
        assert_eq!(build_transactions_filter(false, false), None);
    }
}
