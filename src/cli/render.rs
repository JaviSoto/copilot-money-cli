use comfy_table::modifiers::UTF8_ROUND_CORNERS;
use comfy_table::presets::UTF8_FULL;
use comfy_table::{Attribute, Cell, Color, ContentArrangement, Row as ComfyRow, Table};
use serde::Serialize;
use terminal_size::{Width as TermWidth, terminal_size};
use unicode_width::UnicodeWidthStr;

use super::{Cli, OutputFormat};

#[derive(Debug, Clone, Serialize)]
pub(super) struct KeyValueRow {
    pub key: String,
    pub value: String,
}

pub(super) trait TableRow {
    const HEADERS: &'static [&'static str];
    fn cells(&self) -> Vec<Cell>;
}

pub(super) fn terminal_width() -> Option<u16> {
    if let Ok(cols) = std::env::var("COLUMNS")
        && let Ok(v) = cols.parse::<u16>()
    {
        return Some(v);
    }
    terminal_size().map(|(TermWidth(w), _)| w)
}

pub(super) fn shorten_id_for_table(id: &str) -> String {
    let id = id.trim();
    let max = 18usize;
    if id.is_empty() || id.width() <= max {
        return id.to_string();
    }
    // Keep enough context to copy/paste from JSON output, but make tables readable.
    let prefix_len = 8usize;
    let suffix_len = 6usize;
    if id.len() <= prefix_len + suffix_len + 1 {
        return id.to_string();
    }
    format!("{}…{}", &id[..prefix_len], &id[id.len() - suffix_len..])
}

pub(super) fn render_output<T: Serialize + TableRow>(
    cli: &Cli,
    rows: Vec<T>,
) -> anyhow::Result<()> {
    match cli.output {
        OutputFormat::Json => {
            let s = serde_json::to_string_pretty(&rows)?;
            println!("{s}");
            Ok(())
        }
        OutputFormat::Table => {
            let mut table = Table::new();
            table
                .load_preset(UTF8_FULL)
                .apply_modifier(UTF8_ROUND_CORNERS)
                .set_content_arrangement(ContentArrangement::DynamicFullWidth);

            if let Some(w) = terminal_width() {
                table.set_width(w);
            }

            table.set_header(ComfyRow::from(
                T::HEADERS
                    .iter()
                    .map(|h| header_cell(cli, h))
                    .collect::<Vec<_>>(),
            ));
            for row in rows {
                table.add_row(ComfyRow::from(row.cells()));
            }
            println!("{table}");
            Ok(())
        }
    }
}

pub(super) fn header_cell(cli: &Cli, text: &str) -> Cell {
    if super::should_color(cli) {
        Cell::new(text)
            .add_attribute(Attribute::Bold)
            .fg(Color::Cyan)
    } else {
        Cell::new(text)
    }
}
