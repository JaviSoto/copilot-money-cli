use comfy_table::Cell;
use serde::Serialize;

use crate::client::CopilotClient;

use super::render::{KeyValueRow, TableRow, render_output, shorten_id_for_table};
use super::{Cli, TagsCmd};

pub(super) fn run_tags(cli: &Cli, client: &CopilotClient, cmd: TagsCmd) -> anyhow::Result<()> {
    match cmd {
        TagsCmd::List => {
            let items = client.list_tags()?;
            let rows = items
                .into_iter()
                .map(|t| TagRow {
                    id: t.id,
                    name: t.name.unwrap_or_default(),
                    color_name: t.color_name.unwrap_or_default(),
                })
                .collect::<Vec<_>>();
            render_output(cli, rows)
        }
        TagsCmd::Create(args) => {
            if cli.dry_run {
                println!("dry-run: would create tag: {}", args.name);
                return Ok(());
            }
            super::confirm_write(cli, &format!("Create tag: {}", args.name))?;
            let tag = client.create_tag(&args.name, args.color_name.as_deref())?;
            render_output(
                cli,
                vec![
                    KeyValueRow {
                        key: "id".to_string(),
                        value: tag.id.to_string(),
                    },
                    KeyValueRow {
                        key: "name".to_string(),
                        value: tag.name.unwrap_or_default(),
                    },
                    KeyValueRow {
                        key: "color_name".to_string(),
                        value: tag.color_name.unwrap_or_default(),
                    },
                ],
            )
        }
        TagsCmd::Delete(args) => {
            if cli.dry_run {
                println!("dry-run: would delete tag {}", args.id);
                return Ok(());
            }
            super::confirm_write(cli, &format!("Delete tag {}", args.id))?;
            let ok = client.delete_tag(&args.id)?;
            render_output(
                cli,
                vec![KeyValueRow {
                    key: "deleted".to_string(),
                    value: ok.to_string(),
                }],
            )
        }
    }
}

#[derive(Debug, Clone, Serialize)]
struct TagRow {
    id: crate::types::TagId,
    name: String,
    color_name: String,
}

impl TableRow for TagRow {
    const HEADERS: &'static [&'static str] = &["id", "name", "color_name"];

    fn cells(&self) -> Vec<Cell> {
        vec![
            Cell::new(shorten_id_for_table(self.id.as_str())),
            Cell::new(&self.name),
            Cell::new(&self.color_name),
        ]
    }
}
