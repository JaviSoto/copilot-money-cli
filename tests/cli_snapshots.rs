use assert_cmd::Command;

fn run(args: &[&str]) -> String {
    let tmp_home = tempfile::tempdir().unwrap();
    let mut cmd = Command::new(assert_cmd::cargo::cargo_bin!("copilot"));
    cmd.env("HOME", tmp_home.path());
    cmd.env_remove("COPILOT_TOKEN");
    cmd.env_remove("COPILOT_TOKEN_FILE");
    cmd.env("COPILOT_FIXTURES_DIR", "tests/fixtures/graphql");
    cmd.args(args);
    let out = cmd.assert().success().get_output().stdout.clone();
    String::from_utf8(out).unwrap()
}

#[test]
fn transactions_list_table_snapshot() {
    insta::assert_snapshot!(run(&["transactions", "list"]));
}

#[test]
fn transactions_list_json_snapshot() {
    insta::assert_snapshot!(run(&["--output", "json", "transactions", "list"]));
}

#[test]
fn transactions_list_table_page_info_snapshot() {
    insta::assert_snapshot!(run(&["transactions", "list", "--page-info"]));
}

#[test]
fn transactions_list_json_page_info_snapshot() {
    insta::assert_snapshot!(run(&[
        "--output",
        "json",
        "transactions",
        "list",
        "--page-info"
    ]));
}

#[test]
fn transactions_list_table_filter_tag_snapshot() {
    insta::assert_snapshot!(run(&["transactions", "list", "--tag", "Shopping"]));
}

#[test]
fn transactions_list_table_filter_category_snapshot() {
    insta::assert_snapshot!(run(&["transactions", "list", "--category-id", "cat_other"]));
}

#[test]
fn transactions_search_table_snapshot() {
    insta::assert_snapshot!(run(&["transactions", "search", "amazon"]));
}

#[test]
fn transactions_search_json_snapshot() {
    insta::assert_snapshot!(run(&[
        "--output",
        "json",
        "transactions",
        "search",
        "amazon"
    ]));
}

#[test]
fn transactions_show_table_snapshot() {
    insta::assert_snapshot!(run(&["transactions", "show", "txn_1"]));
}

#[test]
fn transactions_show_json_snapshot() {
    insta::assert_snapshot!(run(&["--output", "json", "transactions", "show", "txn_1"]));
}

#[test]
fn transactions_list_table_fields_and_sort_snapshot() {
    insta::assert_snapshot!(run(&[
        "transactions",
        "list",
        "--fields",
        "date,name,amount,reviewed,category,tags,type",
        "--sort",
        "date-desc",
    ]));
}

#[test]
fn transactions_list_table_filter_reviewed_snapshot() {
    insta::assert_snapshot!(run(&["transactions", "list", "--reviewed"]));
}

#[test]
fn transactions_list_table_filter_unreviewed_snapshot() {
    insta::assert_snapshot!(run(&["transactions", "list", "--unreviewed"]));
}

#[test]
fn transactions_list_table_filter_date_snapshot() {
    insta::assert_snapshot!(run(&["transactions", "list", "--date", "12-15-2025"]));
}

#[test]
fn transactions_set_category_by_name_table_snapshot() {
    insta::assert_snapshot!(run(&[
        "--yes",
        "transactions",
        "set-category",
        "txn_1",
        "--category",
        "Other",
    ]));
}

#[test]
fn transactions_set_notes_table_snapshot() {
    insta::assert_snapshot!(run(&[
        "--yes",
        "transactions",
        "set-notes",
        "txn_1",
        "--notes",
        "hello world",
    ]));
}

#[test]
fn transactions_clear_notes_table_snapshot() {
    insta::assert_snapshot!(run(&[
        "--yes",
        "transactions",
        "set-notes",
        "txn_1",
        "--clear",
    ]));
}

#[test]
fn transactions_set_tags_add_table_snapshot() {
    insta::assert_snapshot!(run(&[
        "--yes",
        "transactions",
        "set-tags",
        "txn_2",
        "--mode",
        "add",
        "--tag-id",
        "tag_shopping",
    ]));
}

#[test]
fn transactions_assign_recurring_table_snapshot() {
    insta::assert_snapshot!(run(&[
        "--yes",
        "transactions",
        "assign-recurring",
        "txn_1",
        "--recurring-id",
        "rec_1",
    ]));
}

#[test]
fn transactions_edit_type_table_snapshot() {
    insta::assert_snapshot!(run(&[
        "--yes",
        "transactions",
        "edit",
        "txn_1",
        "--type",
        "internal-transfer",
    ]));
}
#[test]
fn auth_status_table_snapshot() {
    insta::assert_snapshot!(run(&["auth", "status"]));
}

#[test]
fn auth_status_json_snapshot() {
    insta::assert_snapshot!(run(&["--output", "json", "auth", "status"]));
}

#[test]
fn categories_list_table_snapshot() {
    insta::assert_snapshot!(run(&["categories", "list"]));
}

#[test]
fn categories_list_json_snapshot() {
    insta::assert_snapshot!(run(&["--output", "json", "categories", "list"]));
}

#[test]
fn recurrings_list_table_snapshot() {
    insta::assert_snapshot!(run(&["recurrings", "list"]));
}

#[test]
fn recurrings_list_json_snapshot() {
    insta::assert_snapshot!(run(&["--output", "json", "recurrings", "list"]));
}

#[test]
fn categories_show_table_snapshot() {
    insta::assert_snapshot!(run(&["categories", "show", "cat_other"]));
}

#[test]
fn categories_show_json_snapshot() {
    insta::assert_snapshot!(run(&[
        "--output",
        "json",
        "categories",
        "show",
        "cat_other"
    ]));
}

#[test]
fn categories_create_table_snapshot() {
    insta::assert_snapshot!(run(&[
        "--yes",
        "categories",
        "create",
        "New Category",
        "--emoji",
        "😀",
        "--color-name",
        "BLUE1",
    ]));
}

#[test]
fn recurrings_show_table_snapshot() {
    insta::assert_snapshot!(run(&["recurrings", "show", "rec_1"]));
}

#[test]
fn recurrings_list_filtered_snapshot() {
    insta::assert_snapshot!(run(&["recurrings", "list", "--category-id", "cat_housing"]));
}

#[test]
fn recurrings_create_table_snapshot() {
    insta::assert_snapshot!(run(&[
        "--yes",
        "recurrings",
        "create",
        "txn_1",
        "--frequency",
        "monthly",
    ]));
}

#[test]
fn recurrings_edit_table_snapshot() {
    insta::assert_snapshot!(run(&[
        "--yes",
        "recurrings",
        "edit",
        "rec_1",
        "--name-contains",
        "rent",
        "--min-amount",
        "10",
        "--max-amount",
        "5000",
        "--recalculate-only-for-future",
    ]));
}

#[test]
fn budgets_month_table_snapshot() {
    insta::assert_snapshot!(run(&["budgets", "month"]));
}

#[test]
fn budgets_month_json_snapshot() {
    insta::assert_snapshot!(run(&["--output", "json", "budgets", "month"]));
}
