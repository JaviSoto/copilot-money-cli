use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::config::{load_token, save_token};
use crate::ops;
use crate::types::{
    AccountId, CategoryId, ItemId, RecurringFrequency, RecurringId, TagId, TransactionId,
    TransactionType,
};

#[derive(Debug, Clone)]
pub enum ClientMode {
    Http {
        base_url: String,
        token: Option<String>,
        token_file: PathBuf,
        session_dir: Option<PathBuf>,
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
        Ok(self
            .list_transactions_page(limit, None, None, None)?
            .transactions)
    }

    pub fn list_transactions_page(
        &self,
        first: usize,
        after: Option<String>,
        filter: Option<Value>,
        sort: Option<Value>,
    ) -> anyhow::Result<TransactionsPage> {
        let data = self.graphql(
            "Transactions",
            ops::TRANSACTIONS,
            json!({
                "first": first,
                "after": after,
                "filter": filter,
                "sort": sort,
            }),
        )?;

        let edges = data
            .pointer("/data/transactions/edges")
            .and_then(|v| v.as_array())
            .ok_or_else(|| anyhow::anyhow!("unexpected Transactions response shape"))?;

        let mut transactions = Vec::new();
        for edge in edges {
            if let Some(node) = edge.pointer("/node") {
                let t: Transaction = serde_json::from_value(node.clone())?;
                transactions.push(t);
            }
        }

        let page_info = data
            .pointer("/data/transactions/pageInfo")
            .cloned()
            .unwrap_or_else(|| json!({}));
        let page_info: PageInfo = serde_json::from_value(page_info)?;

        Ok(TransactionsPage {
            transactions,
            page_info,
        })
    }

    pub fn list_categories(
        &self,
        spend: bool,
        budget: bool,
        rollovers: bool,
    ) -> anyhow::Result<Vec<Category>> {
        let data = self.graphql(
            "Categories",
            ops::CATEGORIES,
            json!({
                "spend": spend,
                "budget": budget,
                "rollovers": rollovers
            }),
        )?;

        let items = data
            .pointer("/data/categories")
            .and_then(|v| v.as_array())
            .ok_or_else(|| anyhow::anyhow!("unexpected Categories response shape"))?;

        let mut out = Vec::new();
        for item in items {
            let c: Category = serde_json::from_value(item.clone())?;
            out.push(c);
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

    pub fn list_tags(&self) -> anyhow::Result<Vec<Tag>> {
        let data = self.graphql("Tags", ops::TAGS, json!({}))?;
        let items = data
            .pointer("/data/tags")
            .and_then(|v| v.as_array())
            .ok_or_else(|| anyhow::anyhow!("unexpected Tags response shape"))?;

        let mut out = Vec::new();
        for item in items {
            let t: Tag = serde_json::from_value(item.clone())?;
            out.push(t);
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

    pub fn bulk_edit_transactions_reviewed(
        &self,
        ids: Vec<TransactionIdRef>,
        is_reviewed: bool,
    ) -> anyhow::Result<BulkEditTransactionsResult> {
        let data = self.graphql(
            "BulkEditTransactions",
            ops::BULK_EDIT_TRANSACTIONS,
            json!({
                "filter": { "ids": ids },
                "input": { "isReviewed": is_reviewed }
            }),
        )?;

        let updated = data
            .pointer("/data/bulkEditTransactions/updated")
            .and_then(|v| v.as_array())
            .ok_or_else(|| anyhow::anyhow!("unexpected BulkEditTransactions response shape"))?;
        let mut updated_out = Vec::new();
        for item in updated {
            let t: Transaction = serde_json::from_value(item.clone())?;
            updated_out.push(t);
        }

        let failed_out = match data
            .pointer("/data/bulkEditTransactions/failed")
            .and_then(|v| v.as_array())
        {
            None => Vec::new(),
            Some(items) => {
                let mut out = Vec::new();
                for item in items {
                    let f: BulkEditFailed = serde_json::from_value(item.clone())?;
                    out.push(f);
                }
                out
            }
        };

        Ok(BulkEditTransactionsResult {
            updated: updated_out,
            failed: failed_out,
        })
    }

    pub fn edit_transaction(
        &self,
        item_id: &ItemId,
        account_id: &AccountId,
        id: &TransactionId,
        input: Value,
    ) -> anyhow::Result<Transaction> {
        let data = self.graphql(
            "EditTransaction",
            ops::EDIT_TRANSACTION,
            json!({
                "itemId": item_id.as_str(),
                "accountId": account_id.as_str(),
                "id": id.as_str(),
                "input": input
            }),
        )?;

        let txn = data
            .pointer("/data/editTransaction/transaction")
            .cloned()
            .ok_or_else(|| anyhow::anyhow!("unexpected EditTransaction response shape"))?;
        Ok(serde_json::from_value(txn)?)
    }

    pub fn add_transaction_to_recurring(
        &self,
        item_id: &ItemId,
        account_id: &AccountId,
        id: &TransactionId,
        recurring_id: &RecurringId,
    ) -> anyhow::Result<Transaction> {
        let data = self.graphql(
            "AddTransactionToRecurring",
            ops::ADD_TRANSACTION_TO_RECURRING,
            json!({
                "itemId": item_id.as_str(),
                "accountId": account_id.as_str(),
                "id": id.as_str(),
                "input": {
                    "isExcluded": false,
                    "recurringId": recurring_id.as_str()
                }
            }),
        )?;

        let txn = data
            .pointer("/data/addTransactionToRecurring/transaction")
            .cloned()
            .ok_or_else(|| {
                anyhow::anyhow!("unexpected AddTransactionToRecurring response shape")
            })?;
        Ok(serde_json::from_value(txn)?)
    }

    pub fn delete_tag(&self, id: &TagId) -> anyhow::Result<bool> {
        let data = self.graphql(
            "DeleteTag",
            ops::DELETE_TAG,
            json!({
                "id": id.as_str(),
            }),
        )?;

        let v = data
            .pointer("/data/deleteTag")
            .and_then(|v| v.as_bool())
            .ok_or_else(|| anyhow::anyhow!("unexpected DeleteTag response shape"))?;
        Ok(v)
    }

    pub fn create_tag(&self, name: &str, color_name: Option<&str>) -> anyhow::Result<Tag> {
        let data = self.graphql(
            "CreateTag",
            ops::CREATE_TAG,
            json!({
                "input": {
                    "name": name,
                    "colorName": color_name
                }
            }),
        )?;

        let tag = data
            .pointer("/data/createTag")
            .cloned()
            .ok_or_else(|| anyhow::anyhow!("unexpected CreateTag response shape"))?;
        Ok(serde_json::from_value(tag)?)
    }

    pub fn create_category(
        &self,
        input: Value,
        spend: bool,
        budget: bool,
    ) -> anyhow::Result<Category> {
        let data = self.graphql(
            "CreateCategory",
            ops::CREATE_CATEGORY,
            json!({
                "input": input,
                "spend": spend,
                "budget": budget
            }),
        )?;

        let cat = data
            .pointer("/data/createCategory")
            .cloned()
            .ok_or_else(|| anyhow::anyhow!("unexpected CreateCategory response shape"))?;
        Ok(serde_json::from_value(cat)?)
    }

    pub fn create_recurring_from_transaction(
        &self,
        item_id: &ItemId,
        account_id: &AccountId,
        transaction_id: &TransactionId,
        frequency: RecurringFrequency,
    ) -> anyhow::Result<Recurring> {
        let data = self.graphql(
            "CreateRecurring",
            ops::CREATE_RECURRING,
            json!({
                "input": {
                    "frequency": frequency,
                    "transaction": {
                        "accountId": account_id.as_str(),
                        "itemId": item_id.as_str(),
                        "transactionId": transaction_id.as_str()
                    }
                }
            }),
        )?;

        let recurring = data
            .pointer("/data/createRecurring")
            .cloned()
            .ok_or_else(|| anyhow::anyhow!("unexpected CreateRecurring response shape"))?;
        Ok(serde_json::from_value(recurring)?)
    }

    pub fn edit_recurring(&self, id: &RecurringId, input: Value) -> anyhow::Result<Recurring> {
        let data = self.graphql(
            "EditRecurring",
            ops::EDIT_RECURRING,
            json!({
                "id": id.as_str(),
                "input": input
            }),
        )?;

        let recurring = data
            .pointer("/data/editRecurring/recurring")
            .cloned()
            .ok_or_else(|| anyhow::anyhow!("unexpected EditRecurring response shape"))?;
        Ok(serde_json::from_value(recurring)?)
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
            ClientMode::Http {
                base_url,
                token,
                token_file,
                session_dir,
            } => {
                let url = format!("{}/api/graphql", base_url.trim_end_matches('/'));
                let http = reqwest::blocking::Client::new();

                let mut current_token = token.clone().or_else(|| load_token(token_file).ok());

                for attempt in 1..=2 {
                    let mut req = http.post(&url).json(&json!({
                        "operationName": operation_name,
                        "query": query,
                        "variables": variables
                    }));
                    if let Some(t) = current_token.as_ref() {
                        req = req.bearer_auth(t);
                    }

                    let resp = req.send()?;
                    let status = resp.status();
                    let body: Value = resp.json()?;

                    if is_unauthenticated(&body) {
                        if attempt == 1
                            && let Some(dir) = session_dir.as_ref().filter(|d| d.exists())
                        {
                            let refreshed = refresh_token_via_session(dir, 180)?;
                            save_token(token_file, &refreshed)?;
                            current_token = Some(refreshed);
                            continue;
                        }
                        anyhow::bail!(
                            "unauthenticated (token missing/expired). Re-run `copilot auth login` (or `copilot auth set-token`)."
                        );
                    }

                    if let Some(msg) = format_graphql_error(&body) {
                        anyhow::bail!("{msg}");
                    }

                    if !status.is_success() {
                        anyhow::bail!("graphql http error {status}");
                    }
                    return Ok(body);
                }

                unreachable!("loop returns or errors")
            }
        }
    }
}

fn is_unauthenticated(body: &Value) -> bool {
    body.get("errors")
        .and_then(|e| e.as_array())
        .and_then(|arr| arr.first())
        .and_then(|e| e.get("extensions"))
        .and_then(|ext| ext.get("code"))
        .and_then(|c| c.as_str())
        == Some("UNAUTHENTICATED")
}

fn format_graphql_error(body: &Value) -> Option<String> {
    let errors = body.get("errors")?.as_array()?;
    let first = errors.first()?;
    let message = first.get("message").and_then(|m| m.as_str()).unwrap_or("");
    let code = first
        .get("extensions")
        .and_then(|e| e.get("code"))
        .and_then(|c| c.as_str());

    if message.is_empty() && code.is_none() {
        return None;
    }

    let mut out = String::new();
    out.push_str("graphql error");
    if let Some(c) = code {
        out.push_str(&format!(" ({c})"));
    }
    if !message.is_empty() {
        out.push_str(&format!(": {message}"));
    }
    Some(out)
}

fn refresh_token_via_session(session_dir: &Path, timeout_seconds: u64) -> anyhow::Result<String> {
    // Test hook: allow deterministic refresh without running the browser helper.
    // (Used by unit tests that simulate an expired token + refresh + retry.)
    if let Ok(t) = std::env::var("COPILOT_TEST_REFRESH_TOKEN")
        && !t.trim().is_empty()
    {
        return Ok(t.trim().to_string());
    }

    let helper = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tools/get_token.py");
    let out = std::process::Command::new("python3")
        .arg(helper)
        .args(["--mode", "session"])
        .args(["--user-data-dir", session_dir.to_string_lossy().as_ref()])
        .args(["--timeout-seconds", &timeout_seconds.to_string()])
        .output()?;

    if !out.status.success() {
        anyhow::bail!("token refresh helper failed");
    }
    let token = String::from_utf8(out.stdout)?.trim().to_string();
    if token.is_empty() {
        anyhow::bail!("token refresh helper returned empty token");
    }
    Ok(token)
}

#[derive(Debug, Deserialize, Serialize)]
pub struct PageInfo {
    #[serde(rename = "endCursor")]
    pub end_cursor: Option<String>,
    #[serde(rename = "hasNextPage")]
    pub has_next_page: Option<bool>,
    #[serde(rename = "hasPreviousPage")]
    pub has_previous_page: Option<bool>,
    #[serde(rename = "startCursor")]
    pub start_cursor: Option<String>,
}

#[derive(Debug)]
pub struct TransactionsPage {
    pub transactions: Vec<Transaction>,
    pub page_info: PageInfo,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct Tag {
    pub id: TagId,
    pub name: Option<String>,
    #[serde(rename = "colorName")]
    pub color_name: Option<String>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct Transaction {
    pub id: TransactionId,
    pub date: Option<String>,
    pub name: Option<String>,
    pub amount: Option<Value>,
    #[serde(rename = "itemId")]
    pub item_id: Option<ItemId>,
    #[serde(rename = "type")]
    pub txn_type: Option<TransactionType>,
    #[serde(rename = "isReviewed")]
    pub is_reviewed: Option<bool>,
    #[serde(rename = "categoryId")]
    pub category_id: Option<CategoryId>,
    #[serde(rename = "accountId")]
    pub account_id: Option<AccountId>,
    #[serde(rename = "recurringId")]
    pub recurring_id: Option<RecurringId>,
    #[serde(rename = "userNotes")]
    pub user_notes: Option<String>,
    pub tags: Option<Vec<Tag>>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct TransactionIdRef {
    #[serde(rename = "accountId")]
    pub account_id: AccountId,
    pub id: TransactionId,
    #[serde(rename = "itemId")]
    pub item_id: ItemId,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct BulkEditFailed {
    pub error: Option<String>,
    #[serde(rename = "errorCode")]
    pub error_code: Option<String>,
}

#[derive(Debug)]
pub struct BulkEditTransactionsResult {
    pub updated: Vec<Transaction>,
    pub failed: Vec<BulkEditFailed>,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(tag = "__typename")]
pub enum Icon {
    EmojiUnicode {
        unicode: Option<String>,
    },
    Genmoji {
        id: Option<String>,
        src: Option<String>,
    },
    #[serde(other)]
    Other,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct Category {
    pub id: CategoryId,
    pub name: Option<String>,
    #[serde(rename = "isRolloverDisabled")]
    pub is_rollover_disabled: Option<bool>,
    #[serde(rename = "canBeDeleted")]
    pub can_be_deleted: Option<bool>,
    #[serde(rename = "isExcluded")]
    pub is_excluded: Option<bool>,
    #[serde(rename = "templateId")]
    pub template_id: Option<String>,
    #[serde(rename = "colorName")]
    pub color_name: Option<String>,
    pub icon: Option<Icon>,
    #[serde(rename = "childCategories")]
    pub child_categories: Option<Vec<Category>>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct Recurring {
    pub id: RecurringId,
    pub name: Option<String>,
    pub frequency: Option<RecurringFrequency>,
    #[serde(rename = "categoryId")]
    pub category_id: Option<CategoryId>,
}

#[derive(Debug, Clone)]
pub struct BudgetMonth {
    pub month: String,
    pub amount: String,
}
