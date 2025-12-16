use assert_cmd::Command;

fn run(args: &[&str]) -> String {
    let mut cmd = Command::new(assert_cmd::cargo::cargo_bin!("copilot"));
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
fn transactions_search_table_snapshot() {
    insta::assert_snapshot!(run(&["transactions", "search", "amazon"]));
}

#[test]
fn transactions_show_table_snapshot() {
    insta::assert_snapshot!(run(&["transactions", "show", "txn_1"]));
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
fn budgets_month_table_snapshot() {
    insta::assert_snapshot!(run(&["budgets", "month"]));
}

#[test]
fn budgets_month_json_snapshot() {
    insta::assert_snapshot!(run(&["--output", "json", "budgets", "month"]));
}
