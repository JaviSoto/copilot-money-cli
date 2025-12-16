use std::fs;
use std::path::PathBuf;

use serde::Deserialize;
use serde_json::{Value, json};

use crate::ops;

#[derive(Debug, Clone)]
pub enum ClientMode {
    Http {
        base_url: String,
        token: Option<String>,
    },
    Fixtures(PathBuf),
}

#[derive(Debug, Clone)]
pub struct CopilotClient {
    mode: ClientMode,
}

impl CopilotClient {
    pub fn new(mode: ClientMode) -> Self {
        Self { mode }
    }

    pub fn try_user_query(&self) -> anyhow::Result<()> {
        let _ = self.graphql("User", ops::USER, json!({}))?;
        Ok(())
    }

    pub fn list_transactions(&self, limit: usize) -> anyhow::Result<Vec<Transaction>> {
        let data = self.graphql(
            "Transactions",
            ops::TRANSACTIONS,
            json!({
                "first": limit,
                "after": null,
                "filter": null,
                "sort": null,
            }),
        )?;

        let edges = data
            .pointer("/data/transactions/edges")
            .and_then(|v| v.as_array())
            .ok_or_else(|| anyhow::anyhow!("unexpected Transactions response shape"))?;

        let mut out = Vec::new();
        for edge in edges {
            if let Some(node) = edge.pointer("/node") {
                let t: Transaction = serde_json::from_value(node.clone())?;
                out.push(t);
            }
        }
        Ok(out)
    }

    pub fn list_categories(&self) -> anyhow::Result<Vec<Category>> {
        let data = self.graphql(
            "Categories",
            ops::CATEGORIES,
            json!({
                "spend": false,
                "budget": false,
                "rollovers": false
            }),
        )?;

        let items = data
            .pointer("/data/categories")
            .and_then(|v| v.as_array())
            .ok_or_else(|| anyhow::anyhow!("unexpected Categories response shape"))?;

        let mut out = Vec::new();
        for item in items {
            let id = item
                .get("id")
                .and_then(|v| v.as_str())
                .ok_or_else(|| anyhow::anyhow!("missing category id"))?
                .to_string();
            let name = item
                .get("name")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            out.push(Category { id, name });
        }
        Ok(out)
    }

    pub fn list_recurrings(&self) -> anyhow::Result<Vec<Recurring>> {
        let data = self.graphql("Recurrings", ops::RECURRINGS, json!({ "filter": null }))?;
        let items = data
            .pointer("/data/recurrings")
            .and_then(|v| v.as_array())
            .ok_or_else(|| anyhow::anyhow!("unexpected Recurrings response shape"))?;

        let mut out = Vec::new();
        for item in items {
            let r: Recurring = serde_json::from_value(item.clone())?;
            out.push(r);
        }
        Ok(out)
    }

    pub fn list_budget_months(&self) -> anyhow::Result<Vec<BudgetMonth>> {
        let data = self.graphql("Budgets", ops::BUDGETS, json!({}))?;
        let histories = data
            .pointer("/data/categoriesTotal/budget/histories")
            .and_then(|v| v.as_array())
            .ok_or_else(|| anyhow::anyhow!("unexpected Budgets response shape"))?;

        let mut out = Vec::new();
        for item in histories {
            let month = item
                .get("month")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let amount = item
                .get("amount")
                .map(|v| v.to_string())
                .unwrap_or_else(|| "null".into());
            out.push(BudgetMonth { month, amount });
        }
        Ok(out)
    }

    fn graphql(
        &self,
        operation_name: &str,
        query: &str,
        variables: Value,
    ) -> anyhow::Result<Value> {
        match &self.mode {
            ClientMode::Fixtures(dir) => {
                let path = dir.join(format!("{operation_name}.json"));
                let s = fs::read_to_string(&path)?;
                Ok(serde_json::from_str(&s)?)
            }
            ClientMode::Http { base_url, token } => {
                let url = format!("{}/api/graphql", base_url.trim_end_matches('/'));
                let mut req = reqwest::blocking::Client::new().post(url).json(&json!({
                    "operationName": operation_name,
                    "query": query,
                    "variables": variables
                }));
                if let Some(t) = token {
                    req = req.bearer_auth(t);
                }
                let resp = req.send()?;
                let status = resp.status();
                let body: Value = resp.json()?;
                if !status.is_success() {
                    anyhow::bail!("graphql http error {status}: {body}");
                }
                if let Some(errors) = body.get("errors") {
                    anyhow::bail!("graphql errors: {errors}");
                }
                Ok(body)
            }
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct Transaction {
    pub id: String,
    pub date: Option<String>,
    pub name: Option<String>,
    pub amount: Option<String>,
    #[serde(rename = "isReviewed")]
    pub is_reviewed: Option<bool>,
    #[serde(rename = "categoryId")]
    pub category_id: Option<String>,
}

#[derive(Debug, Clone)]
pub struct Category {
    pub id: String,
    pub name: String,
}

#[derive(Debug, Deserialize)]
pub struct Recurring {
    pub id: String,
    pub name: Option<String>,
    pub frequency: Option<String>,
    #[serde(rename = "categoryId")]
    pub category_id: Option<String>,
}

#[derive(Debug, Clone)]
pub struct BudgetMonth {
    pub month: String,
    pub amount: String,
}
