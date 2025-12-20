use comfy_table::Cell;
use serde::Serialize;

use crate::client::{CopilotClient, Recurring};
use crate::types::{CategoryId, RecurringId};

use super::render::{KeyValueRow, TableRow, render_output, shorten_id_for_table};
use super::{Cli, RecurringsCmd, RecurringsListArgs};

pub(super) fn run_recurrings(
    cli: &Cli,
    client: &CopilotClient,
    cmd: RecurringsCmd,
) -> anyhow::Result<()> {
    match cmd {
        RecurringsCmd::List(args) => {
            let items = client.list_recurrings()?;
            let items = filter_recurrings(items, &args);
            let rows = items
                .into_iter()
                .map(|r| RecurringRow {
                    id: r.id,
                    name: r.name.unwrap_or_default(),
                    frequency: r.frequency.map(|f| f.to_string()).unwrap_or_default(),
                    category_id: r.category_id,
                })
                .collect::<Vec<_>>();
            render_output(cli, rows)
        }
        RecurringsCmd::Show { id } => {
            let items = client.list_recurrings()?;
            let found = items.into_iter().find(|r| r.id == id);
            match found {
                Some(r) => render_output(
                    cli,
                    vec![
                        KeyValueRow {
                            key: "id".to_string(),
                            value: r.id.to_string(),
                        },
                        KeyValueRow {
                            key: "name".to_string(),
                            value: r.name.unwrap_or_default(),
                        },
                        KeyValueRow {
                            key: "frequency".to_string(),
                            value: r.frequency.map(|f| f.to_string()).unwrap_or_default(),
                        },
                        KeyValueRow {
                            key: "category_id".to_string(),
                            value: r
                                .category_id
                                .as_ref()
                                .map(|c| c.to_string())
                                .unwrap_or_default(),
                        },
                    ],
                ),
                None => anyhow::bail!("recurring not found"),
            }
        }
        RecurringsCmd::Create(args) => {
            if cli.dry_run {
                println!(
                    "dry-run: would create recurring from transaction {} (frequency={})",
                    args.transaction_id, args.frequency
                );
                return Ok(());
            }
            super::confirm_write(
                cli,
                &format!(
                    "Create recurring from transaction {} (frequency={})",
                    args.transaction_id, args.frequency
                ),
            )?;

            let txns = super::resolve_transactions_by_ids(
                client,
                std::slice::from_ref(&args.transaction_id),
            )?;
            let txn = txns
                .into_iter()
                .next()
                .ok_or_else(|| anyhow::anyhow!("transaction not found"))?;
            let (item_id, account_id) = super::require_item_and_account(&txn)?;

            let recurring = client.create_recurring_from_transaction(
                &item_id,
                &account_id,
                &txn.id,
                args.frequency,
            )?;

            render_output(
                cli,
                vec![
                    KeyValueRow {
                        key: "id".to_string(),
                        value: recurring.id.to_string(),
                    },
                    KeyValueRow {
                        key: "name".to_string(),
                        value: recurring.name.unwrap_or_default(),
                    },
                    KeyValueRow {
                        key: "frequency".to_string(),
                        value: recurring
                            .frequency
                            .as_ref()
                            .map(|v| v.to_string())
                            .unwrap_or_default(),
                    },
                    KeyValueRow {
                        key: "category_id".to_string(),
                        value: recurring
                            .category_id
                            .as_ref()
                            .map(|v| v.to_string())
                            .unwrap_or_default(),
                    },
                ],
            )
        }
        RecurringsCmd::Edit(args) => {
            if cli.dry_run {
                println!("dry-run: would edit recurring {}", args.id);
                return Ok(());
            }
            super::confirm_write(cli, &format!("Edit recurring {}", args.id))?;

            let mut rule = serde_json::Map::new();
            if let Some(s) = args.name_contains.as_ref() {
                rule.insert(
                    "nameContains".to_string(),
                    serde_json::Value::String(s.clone()),
                );
            }
            if let Some(v) = args.min_amount {
                rule.insert(
                    "minAmount".to_string(),
                    serde_json::Value::Number(serde_json::Number::from(v)),
                );
            }
            if let Some(v) = args.max_amount {
                rule.insert(
                    "maxAmount".to_string(),
                    serde_json::Value::Number(serde_json::Number::from(v)),
                );
            }

            let mut input = serde_json::Map::new();
            if args.recalculate_only_for_future {
                input.insert(
                    "recalculateOnlyForFuture".to_string(),
                    serde_json::Value::Bool(true),
                );
            }
            if !rule.is_empty() {
                input.insert("rule".to_string(), serde_json::Value::Object(rule));
            }

            let recurring = client.edit_recurring(&args.id, serde_json::Value::Object(input))?;
            render_output(
                cli,
                vec![
                    KeyValueRow {
                        key: "id".to_string(),
                        value: recurring.id.to_string(),
                    },
                    KeyValueRow {
                        key: "name".to_string(),
                        value: recurring.name.unwrap_or_default(),
                    },
                    KeyValueRow {
                        key: "frequency".to_string(),
                        value: recurring
                            .frequency
                            .as_ref()
                            .map(|v| v.to_string())
                            .unwrap_or_default(),
                    },
                    KeyValueRow {
                        key: "category_id".to_string(),
                        value: recurring
                            .category_id
                            .as_ref()
                            .map(|v| v.to_string())
                            .unwrap_or_default(),
                    },
                ],
            )
        }
    }
}

#[derive(Debug, Clone, Serialize)]
struct RecurringRow {
    id: RecurringId,
    name: String,
    frequency: String,
    category_id: Option<CategoryId>,
}

impl TableRow for RecurringRow {
    const HEADERS: &'static [&'static str] = &["id", "name", "frequency", "category_id"];

    fn cells(&self) -> Vec<Cell> {
        vec![
            Cell::new(shorten_id_for_table(self.id.as_str())),
            Cell::new(&self.name),
            Cell::new(&self.frequency),
            Cell::new(
                self.category_id
                    .as_ref()
                    .map(|c| shorten_id_for_table(c.as_str()))
                    .unwrap_or_default(),
            ),
        ]
    }
}

fn filter_recurrings(mut items: Vec<Recurring>, args: &RecurringsListArgs) -> Vec<Recurring> {
    if let Some(cat) = args.category_id.as_ref() {
        items.retain(|r| r.category_id.as_ref() == Some(cat));
    }
    if let Some(q) = args.name_contains.as_ref() {
        let q = q.to_lowercase();
        items.retain(|r| r.name.as_deref().unwrap_or("").to_lowercase().contains(&q));
    }
    items
}
