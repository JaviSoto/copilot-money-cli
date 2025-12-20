use comfy_table::Cell;
use serde::Serialize;

use crate::client::CopilotClient;

use super::render::{TableRow, render_output};
use super::{BudgetsCmd, Cli};

pub(super) fn run_budgets(
    cli: &Cli,
    client: &CopilotClient,
    cmd: BudgetsCmd,
) -> anyhow::Result<()> {
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
            render_output(cli, rows)
        }
        BudgetsCmd::Set => anyhow::bail!("budgets set not implemented yet (need mutation doc)"),
    }
}

#[derive(Debug, Clone, Serialize)]
struct BudgetRow {
    month: String,
    amount: String,
}

impl TableRow for BudgetRow {
    const HEADERS: &'static [&'static str] = &["month", "amount"];

    fn cells(&self) -> Vec<Cell> {
        vec![Cell::new(&self.month), Cell::new(&self.amount)]
    }
}
