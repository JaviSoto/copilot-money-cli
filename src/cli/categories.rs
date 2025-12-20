use comfy_table::Cell;
use serde::Serialize;

use crate::client::{Category, CopilotClient};
use crate::types::CategoryId;

use super::render::{KeyValueRow, TableRow, render_output, shorten_id_for_table};
use super::{CategoriesCmd, Cli};

pub(super) fn run_categories(
    cli: &Cli,
    client: &CopilotClient,
    cmd: CategoriesCmd,
) -> anyhow::Result<()> {
    match cmd {
        CategoriesCmd::List(args) => {
            let items = client.list_categories(args.spend, args.budget, args.rollovers)?;
            let mut flat = flatten_categories(&items, args.children);

            if let Some(q) = args.name_contains.as_ref() {
                let q = q.to_lowercase();
                flat.retain(|c| c.name.to_lowercase().contains(&q));
            }

            let rows = flat
                .into_iter()
                .map(|c| CategoryRow {
                    id: c.id,
                    name: c.name,
                    parent_id: c.parent_id,
                    excluded: c.is_excluded.unwrap_or(false).to_string(),
                    can_be_deleted: c.can_be_deleted.unwrap_or(false).to_string(),
                })
                .collect::<Vec<_>>();
            render_output(cli, rows)
        }
        CategoriesCmd::Show { id } => {
            let items = client.list_categories(false, false, false)?;
            let found = flatten_categories(&items, true)
                .into_iter()
                .find(|c| c.id == id);
            match found {
                Some(c) => render_output(
                    cli,
                    vec![
                        KeyValueRow {
                            key: "id".to_string(),
                            value: c.id.to_string(),
                        },
                        KeyValueRow {
                            key: "name".to_string(),
                            value: c.name,
                        },
                        KeyValueRow {
                            key: "parent_id".to_string(),
                            value: c
                                .parent_id
                                .as_ref()
                                .map(|p| p.to_string())
                                .unwrap_or_default(),
                        },
                        KeyValueRow {
                            key: "is_excluded".to_string(),
                            value: c.is_excluded.unwrap_or(false).to_string(),
                        },
                    ],
                ),
                None => anyhow::bail!("category not found"),
            }
        }
        CategoriesCmd::Create(args) => {
            if cli.dry_run {
                println!("dry-run: would create category: {}", args.name);
                return Ok(());
            }
            super::confirm_write(cli, &format!("Create category: {}", args.name))?;

            let mut input = serde_json::json!({
                "name": args.name,
                "emoji": args.emoji,
                "colorName": args.color_name,
                "isExcluded": args.excluded,
                "templateId": args.template_id,
            });

            if let Some(amount) = args.budget_unassigned_amount {
                input["budget"] = serde_json::json!({ "unassignedAmount": amount });
            }

            let want_budget = args.budget_unassigned_amount.is_some();
            let cat = client.create_category(input, true, want_budget)?;

            render_output(
                cli,
                vec![
                    KeyValueRow {
                        key: "id".to_string(),
                        value: cat.id.to_string(),
                    },
                    KeyValueRow {
                        key: "name".to_string(),
                        value: cat.name.unwrap_or_default(),
                    },
                    KeyValueRow {
                        key: "is_excluded".to_string(),
                        value: cat.is_excluded.unwrap_or(false).to_string(),
                    },
                    KeyValueRow {
                        key: "template_id".to_string(),
                        value: cat.template_id.unwrap_or_default(),
                    },
                    KeyValueRow {
                        key: "color_name".to_string(),
                        value: cat.color_name.unwrap_or_default(),
                    },
                ],
            )
        }
        CategoriesCmd::Edit(args) => {
            if cli.dry_run {
                println!(
                    "dry-run: would edit category {} (name={:?})",
                    args.id, args.name
                );
                return Ok(());
            }
            anyhow::bail!("categories edit not implemented yet (need captured mutation document)");
        }
    }
}

#[derive(Debug, Clone, Serialize)]
struct CategoryRow {
    id: CategoryId,
    name: String,
    parent_id: Option<CategoryId>,
    excluded: String,
    can_be_deleted: String,
}

impl TableRow for CategoryRow {
    const HEADERS: &'static [&'static str] =
        &["id", "name", "parent_id", "excluded", "can_be_deleted"];

    fn cells(&self) -> Vec<Cell> {
        vec![
            Cell::new(shorten_id_for_table(self.id.as_str())),
            Cell::new(&self.name),
            Cell::new(
                self.parent_id
                    .as_ref()
                    .map(|p| shorten_id_for_table(p.as_str()))
                    .unwrap_or_default(),
            ),
            Cell::new(&self.excluded),
            Cell::new(&self.can_be_deleted),
        ]
    }
}

#[derive(Debug, Clone, Serialize)]
struct FlatCategory {
    id: CategoryId,
    name: String,
    parent_id: Option<CategoryId>,
    is_excluded: Option<bool>,
    can_be_deleted: Option<bool>,
}

fn flatten_categories(categories: &[Category], include_children: bool) -> Vec<FlatCategory> {
    fn walk(
        out: &mut Vec<FlatCategory>,
        cats: &[Category],
        parent_id: Option<&CategoryId>,
        include_children: bool,
    ) {
        for c in cats {
            out.push(FlatCategory {
                id: c.id.clone(),
                name: c.name.clone().unwrap_or_default(),
                parent_id: parent_id.cloned(),
                is_excluded: c.is_excluded,
                can_be_deleted: c.can_be_deleted,
            });
            if include_children && let Some(children) = c.child_categories.as_ref() {
                walk(out, children, Some(&c.id), include_children);
            }
        }
    }

    let mut out = Vec::new();
    walk(&mut out, categories, None, include_children);
    out
}
